//! The byte-level codec shared by every persistence section (format spec sections 2 and 3):
//! fixed little-endian integers, LEB128 varints, strings, the tagged value encoding, CRC-32C and
//! chunk compression.
//!
//! Decoding goes through [`Reader`], a bounds-checked cursor: every length is compared with the
//! bytes that are actually left before anything is allocated, so hostile input can never make
//! the decoder allocate more than the input's own size (times the compression ratio bound) or
//! read out of bounds.

use compact_str::CompactString;

use crate::value::number::{N, Number};
use crate::value::{ElementProperty, PropertyMap};

/// The format version every file of this build writes and reads.
pub(crate) const FORMAT_VERSION: u32 = 1;

/// The largest uncompressed chunk or record body this build writes or accepts (1 GiB): far
/// above the 1 MiB chunk target, so only a single giant element comes near it.
pub(crate) const MAX_BLOCK_BYTES: usize = 1 << 30;

/// The deepest value nesting the encoder writes; a guard against stack exhaustion on values
/// built through the Rust API (the engine's own writes stay within `traversal.max_value_depth`).
const MAX_ENCODE_DEPTH: usize = 4096;

/// A decode failure inside one section: the byte offset within the section and the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodeError {
    pub(crate) offset: u64,
    pub(crate) reason: String,
}

impl DecodeError {
    pub(crate) fn new(offset: u64, reason: impl Into<String>) -> Self {
        Self {
            offset,
            reason: reason.into(),
        }
    }

    /// The error as a [`super::PersistError::Corrupt`] of `location`, the offset shifted by
    /// `base` (the section's position in its file).
    pub(crate) fn at(self, location: impl Into<String>, base: u64) -> super::PersistError {
        super::PersistError::corrupt(location, base.saturating_add(self.offset), self.reason)
    }
}

pub(crate) type DecodeResult<T> = Result<T, DecodeError>;

/// CRC-32C (Castagnoli) of `bytes`.
#[inline]
pub(crate) fn crc(bytes: &[u8]) -> u32 {
    crc32c::crc32c(bytes)
}

/// CRC-32C continued over `bytes` from the running value `crc`.
#[inline]
pub(crate) fn crc_append(crc: u32, bytes: &[u8]) -> u32 {
    crc32c::crc32c_append(crc, bytes)
}

/// The CRC-32C of `a | b` from the CRC of `a`, the CRC of `b` and the length of `b`: a file
/// whose head is known only after its body (a segment's header) is checksummed without
/// holding the body.
#[inline]
pub(crate) fn crc_combine(crc_a: u32, crc_b: u32, len_b: u64) -> u32 {
    // The snapshot encoder refuses a segment longer than `usize::MAX` (32-bit targets).
    crc32c::crc32c_combine(crc_a, crc_b, usize::try_from(len_b).unwrap_or(usize::MAX))
}

// ---------------------------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------------------------

#[inline]
pub(crate) fn put_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

#[inline]
pub(crate) fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[inline]
pub(crate) fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[inline]
pub(crate) fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[inline]
pub(crate) fn put_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Unsigned LEB128.
#[inline]
pub(crate) fn put_uvar(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// Zigzag + LEB128.
#[inline]
pub(crate) fn put_svar(out: &mut Vec<u8>, value: i64) {
    put_uvar(out, ((value << 1) ^ (value >> 63)) as u64);
}

/// `uvar len` + UTF-8 bytes.
#[inline]
pub(crate) fn put_str(out: &mut Vec<u8>, value: &str) {
    put_uvar(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

/// An optional string: `uvar 0` = none, else `uvar len+1` + bytes (the principal of a commit,
/// an edge label in a journal record).
pub(crate) fn put_opt_str(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        None => put_uvar(out, 0),
        Some(value) => {
            put_uvar(out, value.len() as u64 + 1);
            out.extend_from_slice(value.as_bytes());
        }
    }
}

/// How object keys (and, in segments, labels and property keys) are written: inline strings
/// (journal records) or references into a chunk's string table (segments).
pub(crate) trait KeyWriter {
    fn put_key(&mut self, out: &mut Vec<u8>, key: &str);
}

/// Keys as inline strings.
pub(crate) struct InlineKeys;

impl KeyWriter for InlineKeys {
    #[inline]
    fn put_key(&mut self, out: &mut Vec<u8>, key: &str) {
        put_str(out, key);
    }
}

/// Value tags (format spec 3).
pub(crate) mod tag {
    pub(crate) const NULL: u8 = 0;
    pub(crate) const FALSE: u8 = 1;
    pub(crate) const TRUE: u8 = 2;
    pub(crate) const INT64: u8 = 3;
    pub(crate) const FLOAT64: u8 = 4;
    pub(crate) const STRING: u8 = 5;
    pub(crate) const UUID: u8 = 6;
    pub(crate) const ARRAY: u8 = 7;
    pub(crate) const OBJECT: u8 = 8;
    /// "Absent" in the `before` of a journal SetProperty (never a value).
    pub(crate) const ABSENT: u8 = 255;
}

/// Writes one storable value. Elements and maps with non-string keys are not storable and fail.
pub(crate) fn put_value<K: KeyWriter>(
    out: &mut Vec<u8>,
    value: &ElementProperty,
    keys: &mut K,
) -> Result<(), String> {
    put_value_at(out, value, keys, 0)
}

fn put_value_at<K: KeyWriter>(
    out: &mut Vec<u8>,
    value: &ElementProperty,
    keys: &mut K,
    depth: usize,
) -> Result<(), String> {
    match value {
        ElementProperty::Null => put_u8(out, tag::NULL),
        ElementProperty::Boolean(false) => put_u8(out, tag::FALSE),
        ElementProperty::Boolean(true) => put_u8(out, tag::TRUE),
        ElementProperty::Number(number) => match &number.n {
            N::NegInt(int) => {
                put_u8(out, tag::INT64);
                put_svar(out, *int);
            }
            N::Float(float) => {
                put_u8(out, tag::FLOAT64);
                out.extend_from_slice(&float.into_inner().to_bits().to_le_bytes());
            }
        },
        ElementProperty::String(text) => {
            put_u8(out, tag::STRING);
            put_str(out, text);
        }
        // Disk holds the plain value: the chunks are compressed as a whole (D5, D11).
        ElementProperty::Compressed(text) => {
            put_u8(out, tag::STRING);
            text.with_text(|text| put_str(out, text));
        }
        ElementProperty::Uuid(uuid) => {
            put_u8(out, tag::UUID);
            out.extend_from_slice(&uuid.to_be_bytes());
        }
        ElementProperty::Array(items) => {
            if depth >= MAX_ENCODE_DEPTH {
                return Err(format!(
                    "a value nests deeper than {MAX_ENCODE_DEPTH} levels"
                ));
            }
            put_u8(out, tag::ARRAY);
            put_uvar(out, items.len() as u64);
            for item in items {
                put_value_at(out, item, keys, depth + 1)?;
            }
        }
        ElementProperty::Object(object) => {
            if depth >= MAX_ENCODE_DEPTH {
                return Err(format!(
                    "a value nests deeper than {MAX_ENCODE_DEPTH} levels"
                ));
            }
            put_u8(out, tag::OBJECT);
            put_uvar(out, object.len() as u64);
            for (key, item) in object.iter() {
                keys.put_key(out, key);
                put_value_at(out, item, keys, depth + 1)?;
            }
        }
        ElementProperty::Vertex(_) => return Err("a vertex is not a storable value".into()),
        ElementProperty::Edge(_) => return Err("an edge is not a storable value".into()),
        ElementProperty::Map(_) => {
            return Err("a map with a non-string key is not a storable value".into());
        }
    }
    Ok(())
}

/// Writes a property map: `uvar count | (key | value)*`, insertion order kept.
pub(crate) fn put_properties<K: KeyWriter>(
    out: &mut Vec<u8>,
    properties: Option<&PropertyMap>,
    keys: &mut K,
) -> Result<(), String> {
    let Some(properties) = properties else {
        put_uvar(out, 0);
        return Ok(());
    };
    put_uvar(out, properties.len() as u64);
    for (key, value) in properties.iter() {
        keys.put_key(out, key);
        put_value(out, value, keys).map_err(|reason| format!("property '{key}': {reason}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------------

/// A bounds-checked cursor over one section's bytes.
#[derive(Debug, Clone)]
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// The current offset within the section.
    #[inline]
    pub(crate) fn offset(&self) -> u64 {
        self.pos as u64
    }

    /// Bytes left.
    #[inline]
    pub(crate) fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    #[inline]
    pub(crate) fn error(&self, reason: impl Into<String>) -> DecodeError {
        DecodeError::new(self.offset(), reason)
    }

    #[inline]
    pub(crate) fn bytes(&mut self, len: usize) -> DecodeResult<&'a [u8]> {
        if len > self.remaining() {
            return Err(self.error(format!(
                "{len} bytes needed, {} left (truncated)",
                self.remaining()
            )));
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    #[inline]
    fn array<const L: usize>(&mut self) -> DecodeResult<[u8; L]> {
        let bytes = self.bytes(L)?;
        let mut out = [0u8; L];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    #[inline]
    pub(crate) fn u8(&mut self) -> DecodeResult<u8> {
        Ok(self.array::<1>()?[0])
    }

    #[inline]
    pub(crate) fn u16(&mut self) -> DecodeResult<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    #[inline]
    pub(crate) fn u32(&mut self) -> DecodeResult<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    #[inline]
    pub(crate) fn u64(&mut self) -> DecodeResult<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    #[inline]
    pub(crate) fn i64(&mut self) -> DecodeResult<i64> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    #[inline]
    pub(crate) fn u128_be(&mut self) -> DecodeResult<u128> {
        Ok(u128::from_be_bytes(self.array()?))
    }

    /// Unsigned LEB128; more than 10 bytes or a value beyond `u64` is an error.
    pub(crate) fn uvar(&mut self) -> DecodeResult<u64> {
        let start = self.offset();
        let mut value: u64 = 0;
        for index in 0..10u32 {
            let byte = self.u8()?;
            let bits = u64::from(byte & 0x7F);
            if index == 9 && bits > 1 {
                return Err(DecodeError::new(start, "varint overflows u64"));
            }
            value |= bits << (7 * index);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(DecodeError::new(start, "varint longer than 10 bytes"))
    }

    /// Zigzag + LEB128.
    pub(crate) fn svar(&mut self) -> DecodeResult<i64> {
        let raw = self.uvar()?;
        Ok(((raw >> 1) as i64) ^ -((raw & 1) as i64))
    }

    /// A `uvar` that counts items of at least `min_item_bytes` each: larger than what the
    /// remaining bytes can hold is an error, so the count is safe to preallocate with.
    pub(crate) fn count(&mut self, min_item_bytes: usize) -> DecodeResult<usize> {
        let start = self.offset();
        let count = self.uvar()?;
        let max = self.remaining() / min_item_bytes.max(1);
        if count > max as u64 {
            return Err(DecodeError::new(
                start,
                format!("count {count} exceeds what the remaining bytes can hold"),
            ));
        }
        Ok(count as usize)
    }

    /// `uvar len` + UTF-8 bytes, borrowed.
    pub(crate) fn str(&mut self) -> DecodeResult<&'a str> {
        let start = self.offset();
        let len = self.uvar()?;
        if len > self.remaining() as u64 {
            return Err(DecodeError::new(
                start,
                format!("string of {len} bytes runs past the end"),
            ));
        }
        let bytes = self.bytes(len as usize)?;
        std::str::from_utf8(bytes).map_err(|_| DecodeError::new(start, "invalid UTF-8 in a string"))
    }

    /// See [`put_opt_str`].
    pub(crate) fn opt_str(&mut self) -> DecodeResult<Option<&'a str>> {
        let start = self.offset();
        let len = self.uvar()?;
        if len == 0 {
            return Ok(None);
        }
        let len = len - 1;
        if len > self.remaining() as u64 {
            return Err(DecodeError::new(
                start,
                format!("string of {len} bytes runs past the end"),
            ));
        }
        let bytes = self.bytes(len as usize)?;
        std::str::from_utf8(bytes)
            .map(Some)
            .map_err(|_| DecodeError::new(start, "invalid UTF-8 in a string"))
    }
}

/// How keys are read back: inline strings or references into a chunk's string table.
pub(crate) trait KeyReader {
    fn key(&self, reader: &mut Reader<'_>) -> DecodeResult<CompactString>;
}

impl KeyReader for InlineKeys {
    #[inline]
    fn key(&self, reader: &mut Reader<'_>) -> DecodeResult<CompactString> {
        reader.str().map(CompactString::from)
    }
}

/// Keys as references into a chunk's string table.
pub(crate) struct TableKeys<'t>(pub(crate) &'t [CompactString]);

impl KeyReader for TableKeys<'_> {
    #[inline]
    fn key(&self, reader: &mut Reader<'_>) -> DecodeResult<CompactString> {
        let start = reader.offset();
        let index = reader.uvar()?;
        usize::try_from(index)
            .ok()
            .and_then(|index| self.0.get(index))
            .cloned()
            .ok_or_else(|| {
                DecodeError::new(
                    start,
                    format!("string table reference {index} out of range"),
                )
            })
    }
}

/// Reads one value; containers deeper than `max_depth` are an error.
pub(crate) fn read_value<K: KeyReader>(
    reader: &mut Reader<'_>,
    keys: &K,
    max_depth: usize,
) -> DecodeResult<ElementProperty> {
    let tag = reader.u8()?;
    read_tagged_value(reader, tag, keys, max_depth, 0)
}

/// Reads the value after its `tag` byte.
pub(crate) fn read_tagged_value<K: KeyReader>(
    reader: &mut Reader<'_>,
    tag: u8,
    keys: &K,
    max_depth: usize,
    depth: usize,
) -> DecodeResult<ElementProperty> {
    let start = reader.offset().saturating_sub(1);
    Ok(match tag {
        tag::NULL => ElementProperty::Null,
        tag::FALSE => ElementProperty::Boolean(false),
        tag::TRUE => ElementProperty::Boolean(true),
        tag::INT64 => ElementProperty::Number(Number::from_i64(reader.svar()?)),
        tag::FLOAT64 => {
            let bits = reader.u64()?;
            // Built directly: keeps every bit pattern (NaN payloads) exactly as stored.
            ElementProperty::Number(Number {
                n: N::Float(ordered_float::OrderedFloat(f64::from_bits(bits))),
            })
        }
        tag::STRING => ElementProperty::String(CompactString::from(reader.str()?)),
        tag::UUID => ElementProperty::Uuid(reader.u128_be()?),
        tag::ARRAY => {
            if depth >= max_depth {
                return Err(DecodeError::new(
                    start,
                    format!("value nests deeper than {max_depth} levels"),
                ));
            }
            let count = reader.count(1)?;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                let tag = reader.u8()?;
                items.push(read_tagged_value(reader, tag, keys, max_depth, depth + 1)?);
            }
            ElementProperty::Array(items)
        }
        tag::OBJECT => {
            if depth >= max_depth {
                return Err(DecodeError::new(
                    start,
                    format!("value nests deeper than {max_depth} levels"),
                ));
            }
            let count = reader.count(2)?;
            let mut object = PropertyMap::with_capacity(count);
            for _ in 0..count {
                let key_at = reader.offset();
                let key = keys.key(reader)?;
                let tag = reader.u8()?;
                let value = read_tagged_value(reader, tag, keys, max_depth, depth + 1)?;
                if object.insert(key, value).is_some() {
                    return Err(DecodeError::new(key_at, "duplicate key in an object"));
                }
            }
            ElementProperty::Object(Box::new(object))
        }
        other => {
            return Err(DecodeError::new(
                start,
                format!("unknown value tag {other}"),
            ));
        }
    })
}

/// Reads a property map written by [`put_properties`]; a repeated key is an error.
pub(crate) fn read_properties<K: KeyReader>(
    reader: &mut Reader<'_>,
    keys: &K,
    max_depth: usize,
) -> DecodeResult<PropertyMap> {
    let count = reader.count(2)?;
    let mut properties = PropertyMap::with_capacity(count);
    for _ in 0..count {
        let key_at = reader.offset();
        let key = keys.key(reader)?;
        let value = read_value(reader, keys, max_depth)?;
        if properties.insert(key, value).is_some() {
            return Err(DecodeError::new(key_at, "duplicate property key"));
        }
    }
    Ok(properties)
}

// ---------------------------------------------------------------------------------------------
// Compression
// ---------------------------------------------------------------------------------------------

/// A chunk or record body codec (format spec 2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Codec {
    /// Stored as is.
    None,
    /// LZ4 block format (`lz4_flex`), the default.
    #[default]
    Lz4,
    /// zstd (`ruzstd`, feature `persist-zstd`).
    #[cfg(feature = "persist-zstd")]
    Zstd,
}

impl Codec {
    pub(crate) fn byte(self) -> u8 {
        match self {
            Codec::None => 0,
            Codec::Lz4 => 1,
            #[cfg(feature = "persist-zstd")]
            Codec::Zstd => 2,
        }
    }
}

/// Below this many bytes a block is stored uncompressed.
pub(crate) const COMPRESS_FROM: usize = 4096;

/// Compresses `raw` with `codec` when it is at least [`COMPRESS_FROM`] bytes and compression
/// helps; returns the codec byte actually used and the stored bytes (`None`: store `raw`).
pub(crate) fn compress(raw: &[u8], codec: Codec) -> (u8, Option<Vec<u8>>) {
    if raw.len() < COMPRESS_FROM {
        return (0, None);
    }
    let stored = match codec {
        Codec::None => return (0, None),
        Codec::Lz4 => lz4_flex::block::compress(raw),
        #[cfg(feature = "persist-zstd")]
        Codec::Zstd => {
            ruzstd::encoding::compress_to_vec(raw, ruzstd::encoding::CompressionLevel::Fastest)
        }
    };
    if stored.len() >= raw.len() {
        return (0, None);
    }
    (codec.byte(), Some(stored))
}

/// The LZ4 block format expands at most about 255 times (a match length is extended one byte
/// per 255 bytes); a declared size beyond this bound cannot be genuine.
const LZ4_MAX_RATIO: usize = 255;

/// Decompresses `stored` (codec byte `codec`) into exactly `uncompressed` bytes. The size is
/// checked against [`MAX_BLOCK_BYTES`] and the codec's expansion bound before anything is
/// allocated.
pub(crate) fn decompress(
    codec: u8,
    stored: &[u8],
    uncompressed: usize,
) -> Result<std::borrow::Cow<'_, [u8]>, String> {
    if uncompressed > MAX_BLOCK_BYTES {
        return Err(format!(
            "declared size {uncompressed} exceeds the limit of {MAX_BLOCK_BYTES} bytes"
        ));
    }
    match codec {
        0 => {
            if stored.len() != uncompressed {
                return Err(format!(
                    "uncompressed block of {} bytes declares {uncompressed}",
                    stored.len()
                ));
            }
            Ok(std::borrow::Cow::Borrowed(stored))
        }
        1 => {
            if uncompressed
                > stored
                    .len()
                    .saturating_mul(LZ4_MAX_RATIO)
                    .saturating_add(64)
            {
                return Err(format!(
                    "declared size {uncompressed} is beyond what {} LZ4 bytes can hold",
                    stored.len()
                ));
            }
            let mut out = vec![0u8; uncompressed];
            let written = lz4_flex::block::decompress_into(stored, &mut out)
                .map_err(|err| format!("LZ4: {err}"))?;
            if written != uncompressed {
                return Err(format!(
                    "LZ4 block decodes to {written} bytes, {uncompressed} declared"
                ));
            }
            Ok(std::borrow::Cow::Owned(out))
        }
        #[cfg(feature = "persist-zstd")]
        2 => {
            use std::io::Read;
            let mut source = stored;
            let decoder = ruzstd::decoding::StreamingDecoder::new(&mut source)
                .map_err(|err| format!("zstd: {err}"))?;
            let mut out = Vec::new();
            decoder
                .take(uncompressed as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|err| format!("zstd: {err}"))?;
            if out.len() != uncompressed {
                return Err(format!(
                    "zstd block decodes to {} bytes, {uncompressed} declared",
                    out.len()
                ));
            }
            Ok(std::borrow::Cow::Owned(out))
        }
        #[cfg(not(feature = "persist-zstd"))]
        2 => Err("zstd block (codec 2), but this build has no `persist-zstd` feature".into()),
        other => Err(format!("unknown codec {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip_and_reject_overlong_input() {
        for value in [0u64, 1, 127, 128, 300, u32::MAX as u64, u64::MAX] {
            let mut out = Vec::new();
            put_uvar(&mut out, value);
            assert_eq!(Reader::new(&out).uvar().unwrap(), value);
        }
        for value in [0i64, -1, 1, i64::MIN, i64::MAX] {
            let mut out = Vec::new();
            put_svar(&mut out, value);
            assert_eq!(Reader::new(&out).svar().unwrap(), value);
        }
        assert!(Reader::new(&[0xFF; 11]).uvar().is_err());
        // Ten bytes whose last one carries more than the u64's top bit.
        let mut overflow = vec![0xFF; 9];
        overflow.push(0x02);
        assert!(Reader::new(&overflow).uvar().is_err());
        assert!(Reader::new(&[0x80]).uvar().is_err());
    }

    #[test]
    fn counts_beyond_the_input_are_rejected() {
        let mut out = Vec::new();
        put_uvar(&mut out, 1 << 40);
        assert!(Reader::new(&out).count(1).is_err());
    }

    #[test]
    fn decompress_bounds_the_declared_size() {
        assert!(decompress(1, &[0u8; 4], 1 << 20).is_err());
        assert!(decompress(0, &[1, 2, 3], 4).is_err());
        assert!(decompress(9, &[], 0).is_err());
        assert!(decompress(1, &[0u8; 4], MAX_BLOCK_BYTES + 1).is_err());
    }
}
