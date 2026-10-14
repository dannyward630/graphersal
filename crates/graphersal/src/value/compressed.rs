//! A string kept LZ4-compressed in memory ([`CompressedText`]), the payload of the storage-only
//! value [`ElementProperty::Compressed`](crate::value::ElementProperty::Compressed)
//! (`book/src/users_guide/compressed_properties.md`).
//!
//! A compressed value is **self-contained**: it carries its plain length, its compressed bytes
//! and (by `Arc`) the dictionary it was compressed with, so it decompresses without any graph
//! context, also as a copy in the undo log after the rule that made it changed.
//!
//! Every value is verified once when it is made (the bytes are decompressed again and compared
//! with the input), so reading it back cannot fail for bytes this module produced; the bytes are
//! immutable afterwards. The read paths are therefore infallible: should decompression still
//! fail (memory corruption), the value reads as the empty string and the failure is logged
//! through `tracing` (never a panic, rule 1).
//!
//! Reads that only look at the text ([`CompressedText::with_text`]) decompress into a
//! thread-local scratch buffer, so a comparison or a predicate allocates nothing per item.

use std::cell::Cell;
use std::sync::Arc;

use compact_str::CompactString;

/// The largest dictionary the explicit pass builds: the LZ4 window (64 KiB).
pub(crate) const MAX_DICTIONARY_BYTES: usize = 64 * 1024;

/// A value is kept compressed only when its compressed form is at most this share of the plain
/// size (in percent; D6: 90 %).
pub(crate) const MAX_RATIO_PERCENT: usize = 90;

/// A scratch buffer above this capacity is dropped after use instead of being kept for the
/// next read on this thread.
const KEEP_SCRATCH_BYTES: usize = 1024 * 1024;

thread_local! {
    /// The decompression buffer of [`CompressedText::with_text`], taken out while in use (a
    /// nested read, e.g. comparing two compressed values, gets a fresh one).
    static SCRATCH: Cell<Vec<u8>> = const { Cell::new(Vec::new()) };
}

/// The dictionary of a compression rule: sampled text of the rule's current values (at most
/// 64 KiB, the LZ4 window), shared by every value compressed with it.
///
/// Built by the explicit pass of a rule (define, redefine, `recompress`, load) and never
/// persisted: a load builds it again.
#[doc(hidden)]
pub struct CompressionDictionary {
    bytes: Box<[u8]>,
}

impl CompressionDictionary {
    /// A dictionary of these bytes (the last 64 KiB are kept when longer); `None` when there
    /// is too little text to help (fewer than 16 bytes).
    pub(crate) fn new(mut bytes: Vec<u8>) -> Option<Arc<CompressionDictionary>> {
        if bytes.len() > MAX_DICTIONARY_BYTES {
            bytes.drain(..bytes.len() - MAX_DICTIONARY_BYTES);
        }
        (bytes.len() >= 16).then(|| {
            Arc::new(CompressionDictionary {
                bytes: bytes.into_boxed_slice(),
            })
        })
    }

    /// The dictionary's size in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the dictionary is empty (never, for one this crate built).
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The dictionary bytes.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl std::fmt::Debug for CompressionDictionary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CompressionDictionary({} bytes)", self.bytes.len())
    }
}

/// A string kept LZ4-compressed (block format), with its plain length and the dictionary it was
/// compressed with. Only the graph makes one (a compression rule's write path or pass); it is
/// never a stream, script or file value (see
/// [`ElementProperty::Compressed`](crate::value::ElementProperty::Compressed)).
#[doc(hidden)]
pub struct CompressedText {
    /// The plain text's length in bytes.
    len: u32,
    /// The dictionary the bytes were compressed with, if any.
    dict: Option<Arc<CompressionDictionary>>,
    /// The LZ4 block.
    bytes: Box<[u8]>,
}

impl CompressedText {
    /// Compresses `text` (with `dict` when given), or `None` when the result would not be at
    /// most 90 % of the plain size, when the text is longer than `u32::MAX` bytes, or when the
    /// round trip check fails (never expected).
    pub(crate) fn compress(
        text: &str,
        dict: Option<&Arc<CompressionDictionary>>,
    ) -> Option<CompressedText> {
        let plain = text.as_bytes();
        let len = u32::try_from(plain.len()).ok()?;
        let mut out = vec![0u8; lz4_flex::block::get_maximum_output_size(plain.len())];
        let written = match dict {
            Some(dict) => lz4_flex::block::compress_into_with_dict(plain, &mut out, dict.bytes()),
            None => lz4_flex::block::compress_into(plain, &mut out),
        }
        .ok()?;
        if written.saturating_mul(100) > plain.len().saturating_mul(MAX_RATIO_PERCENT) {
            return None;
        }
        out.truncate(written);
        let compressed = CompressedText {
            len,
            dict: dict.cloned(),
            bytes: out.into_boxed_slice(),
        };
        // Verified once here, so every later read of these immutable bytes succeeds.
        let mut check = Vec::new();
        if compressed.decompress_into(&mut check) && check == plain {
            Some(compressed)
        } else {
            tracing::error!(
                len = plain.len(),
                "compressed property: the round trip check failed; the value stays plain"
            );
            None
        }
    }

    /// The plain text's length in bytes.
    pub fn plain_len(&self) -> usize {
        self.len as usize
    }

    /// The compressed size in bytes (without the dictionary).
    pub fn stored_len(&self) -> usize {
        self.bytes.len()
    }

    /// The dictionary the value was compressed with.
    pub fn dictionary(&self) -> Option<&Arc<CompressionDictionary>> {
        self.dict.as_ref()
    }

    /// Decompresses into `out` (cleared first); `false` when the bytes do not decompress to
    /// exactly the recorded length.
    fn decompress_into(&self, out: &mut Vec<u8>) -> bool {
        out.clear();
        out.resize(self.len as usize, 0);
        let result = match &self.dict {
            Some(dict) => {
                lz4_flex::block::decompress_into_with_dict(&self.bytes, out, dict.bytes())
            }
            None => lz4_flex::block::decompress_into(&self.bytes, out),
        };
        matches!(result, Ok(n) if n == self.len as usize)
    }

    /// Calls `f` with the plain text, decompressed into a thread-local scratch buffer (no
    /// allocation once the buffer has grown). A value that does not decompress (impossible for
    /// bytes this crate made, see the module docs) reads as `""` and is logged.
    // Never inlined: recursive value walkers (equality, hashing, the codecs) call it, and its
    // buffer handling would grow every one of their stack frames.
    #[inline(never)]
    pub fn with_text<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        let mut buffer = SCRATCH.with(Cell::take);
        let text = if self.decompress_into(&mut buffer) {
            match std::str::from_utf8(&buffer) {
                Ok(text) => text,
                Err(_) => {
                    tracing::error!("compressed property: decompressed bytes are not UTF-8");
                    ""
                }
            }
        } else {
            tracing::error!("compressed property: the value does not decompress");
            ""
        };
        let result = f(text);
        if buffer.capacity() <= KEEP_SCRATCH_BYTES {
            SCRATCH.with(|scratch| {
                let kept = scratch.take();
                scratch.set(if kept.capacity() > buffer.capacity() {
                    kept
                } else {
                    buffer
                });
            });
        }
        result
    }

    /// The plain text as an owned string.
    #[inline(never)]
    pub fn text(&self) -> CompactString {
        self.with_text(|text| CompactString::new(text))
    }

    /// Logical equality with another compressed value: equal bytes under the same dictionary
    /// (or none) are equal without decompressing; otherwise the texts are compared.
    pub(crate) fn logical_eq(&self, other: &CompressedText) -> bool {
        if self.len != other.len {
            return false;
        }
        let same_dict = match (&self.dict, &other.dict) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if same_dict && self.bytes == other.bytes {
            return true;
        }
        self.with_text(|a| other.with_text(|b| a == b))
    }

    /// Logical equality with a plain string.
    pub(crate) fn eq_str(&self, text: &str) -> bool {
        self.len as usize == text.len() && self.with_text(|plain| plain == text)
    }

    /// Heap bytes this value owns: the compressed block (the dictionary is shared and counted
    /// once per rule, see `MemoryStats::compression_dictionary_bytes`).
    pub(crate) fn heap_bytes(&self) -> u64 {
        (std::mem::size_of::<CompressedText>() + self.bytes.len()) as u64
    }
}

/// Out of line: `ElementProperty::clone` recurses through nested values, and an inlined copy of
/// this would grow every one of its stack frames.
impl Clone for CompressedText {
    #[inline(never)]
    fn clone(&self) -> Self {
        CompressedText {
            len: self.len,
            dict: self.dict.clone(),
            bytes: self.bytes.clone(),
        }
    }
}

impl std::fmt::Debug for CompressedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.with_text(|text| write!(f, "\"{text}\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(len: usize) -> String {
        let words = [
            "career ",
            "engineer ",
            "Rust ",
            "graph ",
            "database ",
            "team ",
        ];
        let mut out = String::new();
        let mut i = 0;
        while out.len() < len {
            out.push_str(words[i % words.len()]);
            i += 1;
        }
        out.truncate(len);
        out
    }

    #[test]
    fn round_trips_ascii_and_multibyte_text() {
        for text in [sample(1000), "Grüße, 東京, 🎉 ".repeat(40)] {
            let compressed = CompressedText::compress(&text, None).expect("compressible");
            assert_eq!(compressed.plain_len(), text.len());
            assert!(compressed.stored_len() < text.len());
            assert_eq!(compressed.text(), text);
            assert!(compressed.eq_str(&text));
        }
    }

    #[test]
    fn incompressible_text_stays_plain() {
        // A pseudo-random string of distinct bytes: LZ4 cannot reach 90 %.
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let text: String = (0..2000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                char::from(b'!' + (state % 90) as u8)
            })
            .collect();
        assert!(CompressedText::compress(&text, None).is_none());
        assert!(CompressedText::compress("", None).is_none());
    }

    #[test]
    fn dictionary_values_decompress_on_their_own() {
        let dict = CompressionDictionary::new(sample(8000).into_bytes()).expect("dictionary");
        let text = sample(600);
        let with = CompressedText::compress(&text, Some(&dict)).expect("compressible");
        let without = CompressedText::compress(&text, None).expect("compressible");
        assert!(with.stored_len() <= without.stored_len());
        assert_eq!(with.text(), text);
        assert!(with.logical_eq(&without));
        // Nested reads use separate buffers.
        assert!(with.with_text(|a| without.with_text(|b| a == b)));
    }

    #[test]
    fn small_samples_make_no_dictionary() {
        assert!(CompressionDictionary::new(b"short".to_vec()).is_none());
        let big = CompressionDictionary::new(vec![b'x'; MAX_DICTIONARY_BYTES + 10]).expect("dict");
        assert_eq!(big.len(), MAX_DICTIONARY_BYTES);
    }
}
