//! Compression rules in the DSL (book page "Compressed Properties"): plain calls on the source,
//! not steps, in both spellings: `g.define_compression(#{..})`, `g.compressions()`,
//! `g.drop_compression(name)`, `g.recompress(name)`.

use super::saved_query::{fail, graph_fail, optional_text};
use super::*;
use crate::error::ScriptError;
use crate::graph::compression::{Codec, CompressionDefinition};
use crate::graph::definition::{Definition, DefinitionKind};
use crate::graph::{AccessRequest, Action, ElementKind, Resource};

/// The keys `define_compression` accepts.
const KEYS: [&str; 7] = [
    "name",
    "label",
    "path",
    "element",
    "codec",
    "min_bytes",
    "dictionary",
];

/// One rule as a map: name, element, label, path, codec, min_bytes, dictionary and its
/// current stats (compressed_values, plain_bytes, stored_bytes, dictionary_bytes).
pub(crate) fn rule_entry(
    definition: &Definition,
    stats: Option<crate::graph::CompressionStats>,
) -> Option<rhai::Map> {
    let rule = definition.as_compression()?;
    let mut map = rhai::Map::new();
    map.insert("name".into(), definition.name().to_string().into());
    map.insert("element".into(), rule.element().as_str().to_string().into());
    map.insert("label".into(), rule.label().to_string().into());
    map.insert("path".into(), rule.path_text().into());
    map.insert("codec".into(), rule.codec().name().to_string().into());
    map.insert(
        "min_bytes".into(),
        Dynamic::from_int(rule.min_bytes() as i64),
    );
    map.insert("dictionary".into(), Dynamic::from_bool(rule.dictionary()));
    let stats = stats.unwrap_or_default();
    let int = |n: u64| Dynamic::from_int(i64::try_from(n).unwrap_or(i64::MAX));
    map.insert("compressed_values".into(), int(stats.compressed_values));
    map.insert("plain_bytes".into(), int(stats.plain_bytes));
    map.insert("stored_bytes".into(), int(stats.stored_bytes));
    map.insert("dictionary_bytes".into(), int(stats.dictionary_bytes));
    Some(map)
}

impl ScriptGraphTraversalSource {
    /// Asks the session's authorizer for `action` on the compression rule `name` (`None`:
    /// every rule).
    fn require_compression(
        &self,
        operation: &str,
        action: Action<'_>,
        name: Option<&str>,
    ) -> Result<(), Box<EvalAltResult>> {
        self.require(
            operation,
            AccessRequest::new(
                action,
                Resource::definition(Some(DefinitionKind::COMPRESSION), name),
            ),
        )
    }

    fn compression_graph(&self, operation: &str) -> Result<&Arc<dyn AnyGraph>, Box<EvalAltResult>> {
        self.g.as_ref().ok_or_else(|| {
            Box::new(EvalAltResult::ErrorRuntime(
                Dynamic::from(format!("{operation}: no graph associated with this source")),
                rhai::Position::NONE,
            ))
        })
    }

    /// `g.define_compression(#{name, label, path, element?, codec?, min_bytes?, dictionary?})`
    /// / `g.defineCompression(..)`: stores (or replaces) a compression rule. `path` is a string
    /// read as a jpath (`"details.career.cv"`, because it IS a path) or a `jpath(..)`;
    /// `element` is `"vertex"` (default) or `"edge"`, `codec` `"lz4"`, `min_bytes` 256 and
    /// `dictionary` true by default. The explicit pass re-encodes the label's values in the
    /// same unit. Asks for `Create` on the rule (`Update` when it replaces one).
    pub fn define_compression(&mut self, spec: Dynamic) -> Result<(), Box<EvalAltResult>> {
        let operation = "define_compression";
        let actual = type_name_of(&spec);
        let mismatch = |expected: String, actual: String| {
            fail(ScriptError::ArgumentMismatch {
                step_name: operation.to_string(),
                expected,
                actual,
            })
        };
        let Some(mut spec) = spec.try_cast::<rhai::Map>() else {
            return Err(mismatch(
                "a map #{name, label, path, element?, codec?, min_bytes?, dictionary?}".into(),
                actual,
            ));
        };
        if let Some(unknown) = spec.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(mismatch(
                format!("only the keys {}", KEYS.join(", ")),
                format!("the key '{unknown}'"),
            ));
        }
        let mut take = |key: &str| spec.remove(key).unwrap_or(Dynamic::UNIT);
        let name = optional_text(operation, "name", take("name"))?
            .ok_or_else(|| mismatch("the key name (the rule's name)".into(), "no name".into()))?;
        let label = optional_text(operation, "label", take("label"))?.ok_or_else(|| {
            mismatch(
                "the key label (the vertex or edge label)".into(),
                "no label".into(),
            )
        })?;
        let path_value = take("path");
        let path: crate::value::JsonPath =
            if let Some(path) = path_value.read_lock::<crate::value::JsonPath>() {
                path.clone()
            } else if path_value.is_string() {
                path_value
                    .to_string()
                    .parse()
                    .map_err(|err: crate::TraverserError| script_error(Box::new(err)))?
            } else {
                return Err(mismatch(
                    "the key path as a string (\"details.career.cv\") or a jpath(..)".into(),
                    if path_value.is_unit() {
                        "no path".to_string()
                    } else {
                        type_name_of(&path_value)
                    },
                ));
            };
        let element = match optional_text(operation, "element", take("element"))?.as_deref() {
            None | Some("vertex") => ElementKind::Vertex,
            Some("edge") => ElementKind::Edge,
            Some(other) => {
                return Err(mismatch(
                    "element \"vertex\" or \"edge\"".into(),
                    format!("\"{other}\""),
                ));
            }
        };
        let mut rule = CompressionDefinition::with_path(element, label, path);
        if let Some(codec) = optional_text(operation, "codec", take("codec"))? {
            let codec = Codec::from_name(&codec).ok_or_else(|| {
                mismatch(
                    "codec \"lz4\" (the only codec of this version)".into(),
                    format!("\"{codec}\""),
                )
            })?;
            rule = rule.with_codec(codec);
        }
        let min_bytes = take("min_bytes");
        if !min_bytes.is_unit() {
            let value = min_bytes
                .as_int()
                .ok()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| {
                    mismatch(
                        "min_bytes as an integer from 1 to 16777216".into(),
                        dynamic_text(&min_bytes),
                    )
                })?;
            rule = rule.with_min_bytes(value);
        }
        let dictionary = take("dictionary");
        if !dictionary.is_unit() {
            let value = dictionary.as_bool().map_err(|_| {
                mismatch("dictionary as a boolean".into(), type_name_of(&dictionary))
            })?;
            rule = rule.with_dictionary(value);
        }
        let definition = Definition::compression(name.as_str(), rule);
        definition
            .validate()
            .map_err(|err| graph_fail(crate::error::GraphError::from(err)))?;
        let graph = self.compression_graph(operation)?;
        let exists = graph
            .definition(DefinitionKind::COMPRESSION, &name)
            .is_some();
        let action = if exists {
            Action::Update
        } else {
            Action::Create
        };
        self.require_compression(operation, action, Some(&name))?;
        graph
            .set_definitions(vec![definition])
            .map_err(script_error)
    }

    /// `g.compressions()`: every compression rule as a map (name, element, label, path, codec,
    /// min_bytes, dictionary, compressed_values, plain_bytes, stored_bytes, dictionary_bytes),
    /// by name. Asks for `Read` on the rules.
    pub fn compressions(&mut self) -> Result<rhai::Array, Box<EvalAltResult>> {
        self.require_compression("compressions", Action::Read, None)?;
        let graph = self.compression_graph("compressions")?;
        Ok(graph
            .definitions(Some(DefinitionKind::COMPRESSION))
            .iter()
            .filter_map(|definition| {
                rule_entry(definition, graph.compression_stats(definition.name()))
            })
            .map(Dynamic::from_map)
            .collect())
    }

    /// `g.drop_compression(name)` / `g.dropCompression(name)`: removes the rule and decompresses
    /// its values (in the same unit); `false` when there is none. Asks for `Delete` on it.
    pub fn drop_compression(&mut self, name: &str) -> Result<bool, Box<EvalAltResult>> {
        self.require_compression("drop_compression", Action::Delete, Some(name))?;
        self.compression_graph("drop_compression")?
            .remove_definition(DefinitionKind::COMPRESSION, name)
            .map_err(script_error)
    }

    /// `g.recompress(name)`: rebuilds the rule's dictionary from the current values and
    /// re-encodes them; changes no logical data (commits nothing). Fails when there is no such
    /// rule. Asks for `Update` on it.
    pub fn recompress(&mut self, name: &str) -> Result<(), Box<EvalAltResult>> {
        self.require_compression("recompress", Action::Update, Some(name))?;
        self.compression_graph("recompress")?
            .recompress(name)
            .map_err(script_error)
    }
}
