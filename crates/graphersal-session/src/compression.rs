//! Compression rules in the session (`book/src/users_guide/compressed_properties.md`; book page "Compressed
//! Properties"): the operations behind the playground's Catalog ▸ Compression manager, the dev
//! server's `/api/compression*` endpoints and its MCP tools `list_compressions`,
//! `define_compression`, `drop_compression`, `recompress`.
//!
//! Every operation works on the catalog of the graph the session serves (a Store's graph too:
//! a change is a commit like any other, journaled and recorded in the dev server's change
//! feed; a `recompress` changes no data and commits nothing).
//!
//! JSON forms:
//!
//! - an **entry** (`list_compressions`): `{name, element: "vertex"|"edge", label, path, codec,
//!   minBytes, dictionary, stats: {compressedValues, plainBytes, storedBytes,
//!   dictionaryBytes}}`; `path` is the canonical jpath text (`$.details.career.cv`);
//! - a **spec** (`define_compression`): `{name, label, path, element?, codec?, minBytes?,
//!   dictionary?, replaces?}`; `replaces` names the rule an edit started from (a rename stores
//!   the new name and drops the old one in one commit);
//! - **field errors** (a refused define): `[{field, message, help?}]` with `field` one of
//!   `name`, `element`, `label`, `path`, `codec`, `minBytes`, `dictionary` or `general`, the
//!   same shape as the saved queries' editor.

use graphersal::catalog::{Codec, CompressionDefinition};
use graphersal::catalog::{Definition, DefinitionKind};
use graphersal::prelude::*;
use graphersal::{catalog::CompressionStats, storage::ElementKind};
use serde_json::{Value, json};

use crate::Session;
use crate::queries::FieldError;

/// The keys a spec may have.
const KEYS: [&str; 8] = [
    "name",
    "label",
    "path",
    "element",
    "codec",
    "minBytes",
    "dictionary",
    "replaces",
];

/// The entry of one rule (module docs); `stats` is `null` when the storage reports none (a
/// storage without compressed properties keeps the rule but never applies it).
fn entry(name: &str, rule: &CompressionDefinition, stats: Option<CompressionStats>) -> Value {
    let stats = stats.map(|stats| {
        json!({
            "compressedValues": stats.compressed_values,
            "plainBytes": stats.plain_bytes,
            "storedBytes": stats.stored_bytes,
            "dictionaryBytes": stats.dictionary_bytes,
        })
    });
    json!({
        "name": name,
        "element": rule.element().as_str(),
        "label": rule.label(),
        "path": rule.path_text(),
        "codec": rule.codec().name(),
        "minBytes": rule.min_bytes(),
        "dictionary": rule.dictionary(),
        "stats": stats,
    })
}

/// The editor field a catalog error of a compression rule belongs to.
fn field_of(err: &DefinitionError) -> FieldError {
    let mut error = FieldError::of(err);
    error.field = match err {
        DefinitionError::InvalidName { .. } => "name".to_string(),
        DefinitionError::InvalidField { field, .. } => match field.as_str() {
            "label" | "path" => field.clone(),
            "min_bytes" => "minBytes".to_string(),
            _ => "general".to_string(),
        },
        DefinitionError::Conflict { .. } => error.field.clone(),
        _ => "general".to_string(),
    };
    error
}

impl Session {
    /// The compression rules of the graph, by name, with their current stats (module docs).
    pub fn list_compressions(&self) -> Value {
        self.graph.compressions()
    }

    /// The rule `name` as its entry, `null` when there is none.
    pub fn get_compression(&self, name: &str) -> Value {
        self.graph.compression(name)
    }

    /// How many compression rules the graph holds.
    pub fn compression_count(&self) -> usize {
        self.graph.count_kind(DefinitionKind::COMPRESSION)
    }

    /// Defines (or replaces) a compression rule from a spec (module docs), in ONE commit: the
    /// explicit pass re-encodes the label's values in it. Without `replaces` an existing rule
    /// of that name is not overwritten (the form's "New"); with `replaces` naming another
    /// rule, the new name must be free and the old rule is dropped in the same commit (a
    /// rename). Answers `{ok: true, rule: <entry>}` or `{ok: false, errors: [{field, message,
    /// help?}]}`, never fails otherwise.
    pub fn define_compression(&mut self, spec: &Value) -> Value {
        match self.define_compression_checked(spec) {
            Ok(entry) => json!({"ok": true, "rule": entry}),
            Err(errors) => json!({
                "ok": false,
                "errors": errors.iter().map(FieldError::to_value).collect::<Vec<_>>(),
            }),
        }
    }

    fn define_compression_checked(&mut self, spec: &Value) -> Result<Value, Vec<FieldError>> {
        let Some(object) = spec.as_object() else {
            return Err(vec![FieldError::new(
                "general",
                "the rule is a JSON object {name, label, path, element?, codec?, minBytes?, \
                 dictionary?, replaces?}",
            )]);
        };
        if let Some(unknown) = object.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(vec![FieldError::new(
                "general",
                format!("unknown key '{unknown}' (the keys are {})", KEYS.join(", ")),
            )]);
        }
        let mut errors = Vec::new();
        let mut text = |key: &str| match spec.get(key) {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) if text.trim().is_empty() => None,
            Some(Value::String(text)) => Some(text.trim().to_string()),
            Some(_) => {
                errors.push(FieldError::new(key, format!("{key} must be a string")));
                None
            }
        };
        let name = text("name").unwrap_or_default();
        let label = text("label").unwrap_or_default();
        let path = text("path").unwrap_or_default();
        let element = text("element");
        let codec = text("codec");
        let replaces = text("replaces").filter(|old| *old != name);
        let is_edit = spec
            .get("replaces")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
        if name.is_empty() {
            errors.push(FieldError::new(
                "name",
                "a compression rule needs a name (an identifier such as customer_cv)",
            ));
        }
        if label.is_empty() {
            errors.push(FieldError::new(
                "label",
                "name the vertex or edge label the rule applies to",
            ));
        }
        if path.is_empty() {
            errors.push(FieldError::new(
                "path",
                "name the property path, for example cv or details.career.cv",
            ));
        }
        let element = match element.as_deref() {
            None | Some("vertex") => ElementKind::Vertex,
            Some("edge") => ElementKind::Edge,
            Some(other) => {
                errors.push(FieldError::new(
                    "element",
                    format!("'{other}' is not vertex or edge"),
                ));
                ElementKind::Vertex
            }
        };
        let codec = match codec.as_deref() {
            None => Codec::Lz4,
            Some(name) => Codec::from_name(name).unwrap_or_else(|| {
                errors.push(FieldError::new(
                    "codec",
                    format!("unknown codec '{name}' (this version has lz4)"),
                ));
                Codec::Lz4
            }),
        };
        let min_bytes = match spec.get("minBytes") {
            None | Some(Value::Null) => None,
            Some(value) => match value.as_u64().and_then(|n| u32::try_from(n).ok()) {
                Some(n) => Some(n),
                None => {
                    errors.push(FieldError::new(
                        "minBytes",
                        "minBytes is a whole number of bytes (1 to 16777216)",
                    ));
                    None
                }
            },
        };
        let dictionary = match spec.get("dictionary") {
            None | Some(Value::Null) => None,
            Some(Value::Bool(flag)) => Some(*flag),
            Some(_) => {
                errors.push(FieldError::new("dictionary", "dictionary is true or false"));
                None
            }
        };
        let mut rule = None;
        if !path.is_empty() {
            match path.parse::<JsonPath>() {
                Ok(parsed) => {
                    rule = Some(CompressionDefinition::with_path(element, &*label, parsed))
                }
                Err(err) => errors.push(FieldError::new("path", err.to_string())),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let Some(mut rule) = rule else {
            return Err(vec![FieldError::new("path", "the path is missing")]);
        };
        rule = rule.with_codec(codec);
        if let Some(min_bytes) = min_bytes {
            rule = rule.with_min_bytes(min_bytes);
        }
        if let Some(dictionary) = dictionary {
            rule = rule.with_dictionary(dictionary);
        }
        // The name and the rule each on their own, so both wrong fields are reported.
        let mut errors = Vec::new();
        if let Err(err) = Definition::compression(
            name.as_str(),
            CompressionDefinition::with_path(
                element,
                "probe",
                "probe".parse().map_err(|err: graphersal::TraverserError| {
                    vec![FieldError::new("general", err.to_string())]
                })?,
            ),
        )
        .validate()
        {
            errors.push(field_of(&err));
        }
        if let Err(err) = Definition::compression("probe", rule.clone()).validate() {
            errors.push(field_of(&err));
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let definition = Definition::compression(name.as_str(), rule);
        self.graph
            .store_compression(definition, is_edit, replaces.as_deref())?;
        Ok(self.get_compression(&name))
    }

    /// Drops the compression rule `name` (its values are decompressed in the same commit):
    /// `true`, or `false` when there was none.
    pub fn drop_compression(&mut self, name: &str) -> Result<bool, String> {
        self.graph
            .drop_definition(DefinitionKind::COMPRESSION, name)
    }

    /// Re-runs the explicit pass of the rule `name` (a new dictionary from the current values,
    /// every value re-encoded); changes no data, commits nothing. Answers the rule's entry.
    pub fn recompress(&mut self, name: &str) -> Result<Value, String> {
        self.graph.recompress_rule(name)?;
        Ok(self.get_compression(name))
    }
}

/// The compression rules of `g`, by name, as their entries (module docs).
pub fn compressions_in<S: GraphStorage>(g: &S) -> Value {
    Value::Array(
        g.definitions()
            .filter_map(|definition| {
                let rule = definition.as_compression()?;
                Some(entry(
                    definition.name(),
                    rule,
                    g.compression_stats(definition.name()),
                ))
            })
            .collect(),
    )
}

/// The compression rule `name` of `g` as its entry, `null` when there is none.
pub fn compression_in<S: GraphStorage>(g: &S, name: &str) -> Value {
    match g
        .definition(DefinitionKind::COMPRESSION, name)
        .and_then(Definition::as_compression)
    {
        Some(rule) => entry(name, rule, g.compression_stats(name)),
        None => Value::Null,
    }
}

/// Stores the checked compression rule `definition` in `g` in ONE commit, with the form's
/// name rules: a new rule (not `is_edit`) or a rename (`replaces`) needs a free name;
/// `replaces` must still exist and is dropped in the same commit.
pub(crate) fn store_compression<S: GraphStorage>(
    g: &mut S,
    definition: Definition,
    is_edit: bool,
    replaces: Option<&str>,
) -> Result<(), Vec<FieldError>> {
    let name = definition.name().to_string();
    let exists = |g: &S, name: &str| g.definition(DefinitionKind::COMPRESSION, name).is_some();
    if (!is_edit || replaces.is_some()) && exists(g, &name) {
        return Err(vec![FieldError::new(
            "name",
            format!("a compression rule named '{name}' exists already; choose another name"),
        )]);
    }
    if let Some(old) = replaces
        && !exists(g, old)
    {
        return Err(vec![FieldError::new(
            "general",
            format!("the compression rule '{old}' no longer exists (dropped elsewhere?)"),
        )]);
    }
    let stored = g.transaction(|unit| {
        if let Some(old) = replaces {
            unit.remove_definition(DefinitionKind::COMPRESSION, old)?;
        }
        unit.set_definition(definition)?;
        Ok::<(), GraphError>(())
    });
    match stored {
        Ok(()) => Ok(()),
        Err(GraphError::Definition(err)) => Err(vec![field_of(&err)]),
        Err(err) => Err(vec![FieldError::new("general", format_diagnostic(&err))]),
    }
}

/// Re-runs the explicit pass of the rule `name` of `g` (changes no data, commits nothing); an
/// error (no such rule; a storage without compressed properties: `Unsupported`) as its
/// diagnostic text.
pub fn recompress_in<S: GraphStorage>(g: &mut S, name: &str) -> Result<(), String> {
    GraphTraversalSource::new(g)
        .recompress(name)
        .map_err(|err| format_diagnostic(&*err))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_text() -> Session {
        let mut s = Session::new();
        let cv = "Senior engineer, led the platform team in Rust. ".repeat(20);
        let out = s.execute(
            &format!(r#"g.add_v("customer").property("cv", "{cv}").to_list()"#),
            &crate::RunOptions::default(),
        );
        assert!(out.contains("\"ok\":true"), "{out}");
        s
    }

    #[test]
    fn define_list_recompress_rename_and_drop() {
        let mut s = session_with_text();
        let defined = s.define_compression(&json!({
            "name": "cv", "label": "customer", "path": "cv", "minBytes": 64
        }));
        assert_eq!(defined["ok"], true, "{defined}");
        assert_eq!(defined["rule"]["path"], "$.cv");
        assert_eq!(defined["rule"]["stats"]["compressedValues"], 1);
        assert_eq!(s.list_compressions().as_array().unwrap().len(), 1);
        assert_eq!(s.compression_count(), 1);
        let again = s.define_compression(&json!({"name": "cv", "label": "customer", "path": "x"}));
        assert_eq!(again["errors"][0]["field"], "name", "{again}");
        let edited = s.define_compression(&json!({
            "name": "cv2", "label": "customer", "path": "cv", "replaces": "cv",
            "dictionary": false
        }));
        assert_eq!(edited["ok"], true, "{edited}");
        assert_eq!(s.list_compressions()[0]["name"], "cv2");
        assert_eq!(s.recompress("cv2").unwrap()["stats"]["compressedValues"], 1);
        assert!(s.recompress("nope").is_err());
        assert!(s.drop_compression("cv2").unwrap());
        assert!(!s.drop_compression("cv2").unwrap());
    }

    #[test]
    fn field_errors_name_their_inputs() {
        let mut s = session_with_text();
        let refused = s.define_compression(&json!({
            "name": "bad name", "label": "", "path": "a[0]", "element": "node",
            "minBytes": -1, "codec": "zip", "dictionary": "yes"
        }));
        assert_eq!(refused["ok"], false);
        let fields: Vec<&str> = refused["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["field"].as_str().unwrap())
            .collect();
        for field in ["label", "element", "codec", "minBytes", "dictionary"] {
            assert!(fields.contains(&field), "{field}: {refused}");
        }
        let refused = s.define_compression(&json!({
            "name": "bad name", "label": "customer", "path": "a[0]"
        }));
        assert_eq!(refused["errors"][0]["field"], "name", "{refused}");
        assert_eq!(refused["errors"][1]["field"], "path", "{refused}");
        let refused = s.define_compression(&json!({
            "name": "bad name", "label": "customer", "path": "a"
        }));
        assert_eq!(refused["errors"][0]["field"], "name", "{refused}");
        assert!(refused["errors"][0]["help"].is_string());
        s.define_compression(&json!({"name": "a", "label": "customer", "path": "cv"}));
        let conflict =
            s.define_compression(&json!({"name": "b", "label": "customer", "path": "cv"}));
        assert_eq!(conflict["errors"][0]["field"], "path", "{conflict}");
        assert!(
            conflict["errors"][0]["help"]
                .as_str()
                .unwrap()
                .contains("drop_compression")
        );
    }

    /// The modern graph twice: as a `TraversalGraph` and wrapped in the `Forwarding` storage
    /// (not `TraversalGraph`, own handle types), for the generic-helper comparisons.
    fn modern_pair() -> (
        TraversalGraph,
        graphersal_storage_tests::forwarding::Forwarding,
    ) {
        (
            TraversalGraph::tinkerpop_modern(),
            graphersal_storage_tests::forwarding::Forwarding::new(
                TraversalGraph::tinkerpop_modern(),
            ),
        )
    }

    #[test]
    fn rules_on_a_foreign_storage_equal_those_of_a_traversal_graph() {
        let (mut tg, mut fw) = modern_pair();
        let cv = "Senior engineer, led the platform team in Rust. ".repeat(20);
        let mut properties = PropertyMap::default();
        properties.insert("cv", cv.as_str().into());
        tg.add_vertex(None, vec!["customer".into()], properties.clone())
            .unwrap();
        fw.add_vertex(None, vec!["customer".into()], properties)
            .unwrap();
        let rule = || {
            let rule = CompressionDefinition::with_path(
                ElementKind::Vertex,
                "customer",
                "cv".parse().unwrap(),
            )
            .with_min_bytes(64);
            Definition::compression("cv", rule)
        };
        store_compression(&mut tg, rule(), false, None).unwrap();
        store_compression(&mut fw, rule(), false, None).unwrap();
        assert_eq!(compressions_in(&tg), compressions_in(&fw));
        assert_eq!(compression_in(&fw, "cv")["stats"]["compressedValues"], 1);
        assert_eq!(compression_in(&fw, "none"), Value::Null);
        recompress_in(&mut tg, "cv").unwrap();
        recompress_in(&mut fw, "cv").unwrap();
        assert_eq!(compression_in(&tg, "cv"), compression_in(&fw, "cv"));
        assert!(recompress_in(&mut fw, "none").is_err());
        // A storage that reports no stats (no compressed properties): the entry says `null`.
        let entry = entry("cv", rule().as_compression().unwrap(), None);
        assert_eq!(entry["stats"], Value::Null);
        assert_eq!(entry["path"], "$.cv");
    }
}
