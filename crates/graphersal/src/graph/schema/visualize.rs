//! Renderers of a [`GraphSchema`]: table, markdown, tree, JSON, JSON Schema, Mermaid and
//! PlantUML. Every renderer shows nested objects (never a bare `object`) and the constraints.

use std::collections::{HashMap, HashSet};

use super::GraphSchema;
use super::nested::number_text;
use super::node::{JsonType, PropertySchema};
use super::parse::pretty_json;
use crate::value::{ElementProperty, PropertyMap};

/// Output format of the schema and result visualizers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VisualizeFormat {
    /// Picks the best format for the target: a table for a terminal, JSON for a script value.
    Auto,
    /// An ASCII/Unicode table (`display` feature).
    Table,
    /// A Markdown table.
    Markdown,
    /// Plain JSON (for a schema: its canonical format).
    Json,
    /// JSON Schema (schema visualization only: one standalone JSON Schema per label).
    JsonSchema,
    /// Mermaid diagram source.
    Mermaid,
    /// PlantUML diagram source.
    PlantUML,
    /// Indented hierarchical tree (Unix `tree`-style box drawing).
    Tree,
}

impl VisualizeFormat {
    /// Returns the canonical lowercase name of the format, as accepted by [`Self::from_name`].
    pub fn as_name(&self) -> &'static str {
        match self {
            VisualizeFormat::Auto => "auto",
            VisualizeFormat::Table => "table",
            VisualizeFormat::Markdown => "markdown",
            VisualizeFormat::Json => "json",
            VisualizeFormat::JsonSchema => "jsonschema",
            VisualizeFormat::Mermaid => "mermaid",
            VisualizeFormat::PlantUML => "plantuml",
            VisualizeFormat::Tree => "tree",
        }
    }

    /// Parses a format from its name, ignoring case and accepting the `json_schema` alias.
    ///
    /// Returns `None` for an unknown name, so callers can report the supported set themselves.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "auto" => Some(VisualizeFormat::Auto),
            "table" => Some(VisualizeFormat::Table),
            "markdown" => Some(VisualizeFormat::Markdown),
            "json" => Some(VisualizeFormat::Json),
            "jsonschema" | "json_schema" => Some(VisualizeFormat::JsonSchema),
            "mermaid" => Some(VisualizeFormat::Mermaid),
            "plantuml" => Some(VisualizeFormat::PlantUML),
            "tree" => Some(VisualizeFormat::Tree),
            _ => None,
        }
    }

    /// Lists every supported format name, in the order they are documented.
    pub fn names() -> &'static [&'static str] {
        &[
            "auto",
            "table",
            "markdown",
            "json",
            "jsonschema",
            "mermaid",
            "plantuml",
            "tree",
        ]
    }
}

/// The constraints of a node, compact: `0..150`, `> 0`, `length 1..80`, `pattern ^x$`,
/// `items <= 10`, `one of ["a", "b"]`, `= "human"`, `additional: false`, `deprecated`.
pub(crate) fn constraint_text(node: &PropertySchema) -> String {
    let mut parts = Vec::new();
    if let Some(c) = node.constraints.as_deref() {
        match (&c.minimum, &c.maximum) {
            (Some(min), Some(max)) => {
                parts.push(format!("{}..{}", number_text(min), number_text(max)))
            }
            (Some(min), None) => parts.push(format!(">= {}", number_text(min))),
            (None, Some(max)) => parts.push(format!("<= {}", number_text(max))),
            (None, None) => {}
        }
        if let Some(min) = &c.exclusive_minimum {
            parts.push(format!("> {}", number_text(min)));
        }
        if let Some(max) = &c.exclusive_maximum {
            parts.push(format!("< {}", number_text(max)));
        }
        match (&c.format_minimum, &c.format_maximum) {
            (Some(min), Some(max)) => parts.push(format!("{min}..{max}")),
            (Some(min), None) => parts.push(format!(">= {min}")),
            (None, Some(max)) => parts.push(format!("<= {max}")),
            (None, None) => {}
        }
        if let Some(min) = &c.format_exclusive_minimum {
            parts.push(format!("> {min}"));
        }
        if let Some(max) = &c.format_exclusive_maximum {
            parts.push(format!("< {max}"));
        }
        let range = |what: &str, min: Option<u64>, max: Option<u64>| match (min, max) {
            (Some(min), Some(max)) => Some(format!("{what} {min}..{max}")),
            (Some(min), None) => Some(format!("{what} >= {min}")),
            (None, Some(max)) => Some(format!("{what} <= {max}")),
            (None, None) => None,
        };
        parts.extend(range("length", c.min_length, c.max_length));
        if let Some(pattern) = &c.pattern {
            parts.push(format!("pattern {pattern}"));
        }
        parts.extend(range("items", c.min_items, c.max_items));
        if let Some(values) = &c.enum_values {
            parts.push(format!(
                "one of {}",
                json_text(&ElementProperty::Array(values.clone()))
            ));
        }
        if let Some(value) = &c.const_value {
            parts.push(format!("= {}", json_text(value)));
        }
    }
    if let Some(allowed) = node.additional_properties {
        parts.push(format!("additional: {allowed}"));
    }
    if let Some(metric) = node.vector.as_ref().and_then(|vector| vector.metric) {
        parts.push(format!("metric {metric}"));
    }
    if node.annotations().and_then(|a| a.deprecated) == Some(true) {
        parts.push("deprecated".to_string());
    }
    parts.join(", ")
}

/// Title, description and `default`/`defaultFunction` of a node. A `default` is applied on
/// create only on a label's `top_level` property and never when it is `null`; elsewhere it is
/// shown as the annotation it is.
fn note_text(node: &PropertySchema, top_level: bool) -> String {
    let Some(a) = node.annotations() else {
        return String::new();
    };
    let mut parts = Vec::new();
    if let Some(title) = &a.title {
        parts.push(title.clone());
    }
    if let Some(description) = &a.description {
        parts.push(description.replace('\n', " "));
    }
    if let Some(default) = &a.default {
        if top_level && !matches!(default, crate::value::ElementProperty::Null) {
            parts.push(format!("default {}", json_text(default)));
        } else {
            parts.push(format!("default {} (annotation)", json_text(default)));
        }
    }
    if let Some(function) = a.default_function {
        parts.push(format!("defaultFunction {}", function.as_str()));
    }
    parts.join("; ")
}

/// One flattened property row.
struct Row<'a> {
    /// `name`, `config.runtime.cpu`, `tags[]`, `items[].a`.
    path: String,
    node: &'a PropertySchema,
    required: bool,
}

/// The nested nodes worth a row below `node`: object keys, array items, and the keys and items
/// of `anyOf` members (a Pydantic `Optional[Model]` shows the model's keys).
fn children(node: &PropertySchema) -> Vec<(String, &PropertySchema, bool, bool)> {
    // (suffix, node, required, is_items)
    let mut out = Vec::new();
    for (key, child) in &node.properties {
        out.push((key.clone(), child, node.is_required(key), false));
    }
    if let Some(items) = &node.items
        && (!items.properties.is_empty() || items.items.is_some() || items.any_of.is_some())
    {
        out.push(("[]".to_string(), items.as_ref(), true, true));
    }
    if let Some(members) = &node.any_of {
        for member in members {
            out.extend(children(member));
        }
    }
    out
}

fn flatten<'a>(node: &'a PropertySchema, prefix: &str, rows: &mut Vec<Row<'a>>) {
    for (suffix, child, required, is_items) in children(node) {
        let path = if is_items {
            format!("{prefix}[]")
        } else if prefix.is_empty() {
            suffix
        } else {
            format!("{prefix}.{suffix}")
        };
        if is_items {
            // The items themselves are described by the array's row; show their keys.
            flatten(child, &path, rows);
        } else {
            rows.push(Row {
                path: path.clone(),
                node: child,
                required,
            });
            flatten(child, &path, rows);
        }
    }
}

/// The label cell: the name, with the root's explicit `additionalProperties` and description.
fn label_cell(label: &str, root: &PropertySchema) -> String {
    match root.additional_properties {
        Some(allowed) => format!("{label} (additional: {allowed})"),
        None => label.to_string(),
    }
}

/// One row of the flattened schema ([`GraphSchema::property_rows`]): a property at any depth of
/// a label, with its type, requiredness, nullability, constraints and notes as short texts. The
/// table, markdown and PlantUML renderers and the playground's schema table are built from it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SchemaRow {
    /// `"vertex"` or `"edge"`.
    pub element: &'static str,
    /// The label, with the root's explicit `additionalProperties` when it states one.
    pub label: String,
    /// An edge label's connections (`a -> b, ...`, `any` without any); empty for a vertex.
    pub connections: String,
    /// The property path: `name`, `address.zip`, `roles[].name`; `-` for a label without
    /// properties.
    pub property: String,
    /// The type without its `null` alternative (`int64`, `array<string>`, `object`).
    pub ty: String,
    /// `yes`/`no` (`-` for a label without properties).
    pub required: String,
    /// `yes`/`no`: whether the type admits `null`.
    pub nullable: String,
    /// The constraints, compact (`0..150`, `length 1..80`, `pattern ^x$`); `-` without any.
    pub constraints: String,
    /// Title, description, `default` (`(annotation)` where it is not applied: `null`, nested)
    /// and `defaultFunction`.
    pub notes: String,
}

impl GraphSchema {
    /// The schema flattened to one [`SchemaRow`] per property path, vertex labels first, then
    /// edge labels, in declaration order. A label whose root node has a title, description or
    /// `default` gets a label row first (property `-`, the notes); a label without properties
    /// gets exactly that row.
    pub fn property_rows(&self) -> Vec<SchemaRow> {
        let mut out = Vec::new();
        let mut label_rows =
            |element: &'static str, label: &str, connections: String, root: &PropertySchema| {
                let mut rows = Vec::new();
                flatten(root, "", &mut rows);
                let label_text = label_cell(label, root);
                let label_notes = note_text(root, false);
                // The label's own title/description: its own row before the property rows
                // (a label without properties gets one row anyway, below).
                if !rows.is_empty() && !label_notes.is_empty() {
                    out.push(SchemaRow {
                        element,
                        label: label_text.clone(),
                        connections: connections.clone(),
                        property: "-".to_string(),
                        ty: "-".to_string(),
                        required: "-".to_string(),
                        nullable: "-".to_string(),
                        constraints: "-".to_string(),
                        notes: label_notes.clone(),
                    });
                }
                if rows.is_empty() {
                    out.push(SchemaRow {
                        element,
                        label: label_text,
                        connections,
                        property: "-".to_string(),
                        ty: "-".to_string(),
                        required: "-".to_string(),
                        nullable: "-".to_string(),
                        constraints: "-".to_string(),
                        notes: label_notes,
                    });
                    return;
                }
                for row in rows {
                    let constraints = constraint_text(row.node);
                    let notes = note_text(row.node, !row.path.contains(['.', '[']));
                    out.push(SchemaRow {
                        element,
                        label: label_text.clone(),
                        connections: connections.clone(),
                        property: row.path,
                        ty: row.node.non_null_type_name(),
                        required: if row.required { "yes" } else { "no" }.to_string(),
                        nullable: if row.node.admits_null() { "yes" } else { "no" }.to_string(),
                        constraints: if constraints.is_empty() {
                            "-".to_string()
                        } else {
                            constraints
                        },
                        notes,
                    });
                }
            };
        for (label, vertex) in &self.vertices {
            label_rows("vertex", label, String::new(), &vertex.schema);
        }
        for (label, edge) in &self.edges {
            let connections = if edge.connections.is_empty() {
                "any".to_string()
            } else {
                edge.connections
                    .iter()
                    .map(|c| format!("{} -> {}", c.from, c.to))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            label_rows("edge", label, connections, &edge.schema);
        }
        out
    }

    fn heading(&self) -> String {
        let mut text = format!("Schema (mode: {})", self.mode);
        if let Some(meta) = &self.meta {
            text.push_str(&format!(" meta: {}", json_text(meta)));
        }
        text
    }

    /// Visualizes the schema in the specified format.
    pub fn visualize(&self, format: VisualizeFormat) -> String {
        match format {
            VisualizeFormat::Auto => {
                #[cfg(feature = "display")]
                {
                    self.to_table()
                }
                #[cfg(not(feature = "display"))]
                {
                    self.to_tree()
                }
            }
            VisualizeFormat::Table => self.to_table(),
            VisualizeFormat::Markdown => self.to_markdown(),
            VisualizeFormat::Json => self.to_json_pretty(),
            VisualizeFormat::JsonSchema => self.to_json_schema(),
            VisualizeFormat::Mermaid => self.to_mermaid(),
            VisualizeFormat::PlantUML => self.to_plantuml(),
            VisualizeFormat::Tree => self.to_tree(),
        }
    }

    /// The schema as Markdown tables (vertices, edges): one row per property path, nested
    /// keys as `config.runtime.cpu`, array items as `tags[]`.
    pub fn to_markdown(&self) -> String {
        let escape = |s: &str| s.replace('|', "\\|").replace('\n', " ");
        let rows = self.property_rows();
        let mut out = format!("**{}**\n\n", escape(&self.heading()));
        for (element, title, with_connections) in [
            ("vertex", "### Vertices", false),
            ("edge", "### Edges", true),
        ] {
            out.push_str(title);
            out.push_str("\n\n");
            let selected: Vec<&SchemaRow> = rows.iter().filter(|r| r.element == element).collect();
            if selected.is_empty() {
                out.push_str(&format!("*(no {element} labels)*\n\n"));
                continue;
            }
            if with_connections {
                out.push_str("| Label | Connections | Property | Type | Required | Nullable | Constraints | Notes |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n");
            } else {
                out.push_str("| Label | Property | Type | Required | Nullable | Constraints | Notes |\n| --- | --- | --- | --- | --- | --- | --- |\n");
            }
            for row in selected {
                let mut cells = vec![escape(&row.label)];
                if with_connections {
                    cells.push(escape(&row.connections));
                }
                cells.extend([
                    escape(&row.property),
                    escape(&row.ty),
                    row.required.clone(),
                    row.nullable.clone(),
                    escape(&row.constraints),
                    escape(&row.notes),
                ]);
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
            out.push('\n');
        }
        out.trim_end().to_string()
    }

    /// The schema as text tables (vertices, edges), like [`Self::to_markdown`] (`display`
    /// feature).
    pub fn to_table(&self) -> String {
        #[cfg(feature = "display")]
        {
            use tabled::builder::Builder;
            use tabled::settings::Style;
            let rows = self.property_rows();
            let mut out = format!("{}\n", self.heading());
            for (element, title, with_connections) in [
                ("vertex", "=== Vertices ===", false),
                ("edge", "=== Edges ===", true),
            ] {
                out.push_str(title);
                out.push('\n');
                let selected: Vec<&SchemaRow> =
                    rows.iter().filter(|r| r.element == element).collect();
                if selected.is_empty() {
                    out.push_str(&format!("(no {element} labels)\n\n"));
                    continue;
                }
                let mut builder = Builder::default();
                let mut header = vec!["Label"];
                if with_connections {
                    header.push("Connections");
                }
                header.extend([
                    "Property",
                    "Type",
                    "Required",
                    "Nullable",
                    "Constraints",
                    "Notes",
                ]);
                builder.push_record(header);
                for row in selected {
                    let mut cells = vec![row.label.clone()];
                    if with_connections {
                        cells.push(row.connections.clone());
                    }
                    cells.extend([
                        row.property.clone(),
                        row.ty.clone(),
                        row.required.clone(),
                        row.nullable.clone(),
                        row.constraints.clone(),
                        row.notes.clone(),
                    ]);
                    builder.push_record(cells);
                }
                let mut table = builder.build();
                table.with(Style::rounded());
                out.push_str(&table.to_string());
                out.push_str("\n\n");
            }
            out.trim_end().to_string()
        }
        #[cfg(not(feature = "display"))]
        {
            "Table display requires the 'display' feature flag".to_string()
        }
    }

    /// The schema as an indented tree: labels, then their properties at every depth.
    pub fn to_tree(&self) -> String {
        fn line(node: &PropertySchema, name: &str, required: bool, top_level: bool) -> String {
            let mut text = format!("{name}: {}", node.non_null_type_name());
            let mut notes = Vec::new();
            if required {
                notes.push("required".to_string());
            }
            if node.admits_null() {
                notes.push("nullable".to_string());
            }
            let constraints = constraint_text(node);
            if !constraints.is_empty() {
                notes.push(constraints);
            }
            let extra = note_text(node, top_level);
            if !extra.is_empty() {
                notes.push(extra);
            }
            if !notes.is_empty() {
                text.push_str(&format!(" ({})", notes.join(", ")));
            }
            text
        }
        fn walk(node: &PropertySchema, prefix: &str, out: &mut Vec<String>) {
            let kids = children(node);
            let count = kids.len();
            for (index, (name, child, required, is_items)) in kids.into_iter().enumerate() {
                let last = index + 1 == count;
                let branch = if last { "└── " } else { "├── " };
                let text = if is_items {
                    format!("[] items: {}", child.non_null_type_name())
                } else {
                    line(child, &name, required, prefix.chars().count() == 4)
                };
                out.push(format!("{prefix}{branch}{text}"));
                let deeper = format!("{prefix}{}", if last { "    " } else { "│   " });
                walk(child, &deeper, out);
            }
        }
        let mut out = vec![self.heading()];
        let mut labels: Vec<(String, &PropertySchema)> = Vec::new();
        for (label, vertex) in &self.vertices {
            labels.push((
                format!("vertex {}", label_cell(label, &vertex.schema)),
                &vertex.schema,
            ));
        }
        for (label, edge) in &self.edges {
            let connections = if edge.connections.is_empty() {
                String::new()
            } else {
                format!(
                    " [{}]",
                    edge.connections
                        .iter()
                        .map(|c| format!("{} -> {}", c.from, c.to))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            labels.push((
                format!("edge {}{connections}", label_cell(label, &edge.schema)),
                &edge.schema,
            ));
        }
        let count = labels.len();
        for (index, (title, root)) in labels.into_iter().enumerate() {
            let last = index + 1 == count;
            out.push(format!("{}{title}", if last { "└── " } else { "├── " }));
            walk(root, if last { "    " } else { "│   " }, &mut out);
        }
        out.join("\n")
    }

    /// One standalone JSON Schema (Draft 2020-12) per label under `$defs` (`vertex:person`,
    /// `edge:knows`), each with the mode's default `additionalProperties` written out, so it
    /// validates a property map on its own.
    pub fn to_json_schema(&self) -> String {
        let default = (self.mode != super::SchemaMode::None)
            .then(|| self.mode.default_additional_properties());
        let mut defs = PropertyMap::default();
        for (label, vertex) in &self.vertices {
            defs.insert(
                format!("vertex:{label}"),
                explicit_additional(&vertex.schema, default).to_value(),
            );
        }
        for (label, edge) in &self.edges {
            defs.insert(
                format!("edge:{label}"),
                explicit_additional(&edge.schema, default).to_value(),
            );
        }
        let mut root = PropertyMap::default();
        root.insert(
            "$schema",
            ElementProperty::from("https://json-schema.org/draft/2020-12/schema"),
        );
        root.insert("$defs", ElementProperty::Object(Box::new(defs)));
        pretty_json(&ElementProperty::Object(Box::new(root)))
    }

    /// A Mermaid `erDiagram`: one entity per label with its properties; a nested object (also
    /// the object items of an array) is an entity of its own (`person_address`) linked to its
    /// parent (`person ||--|| person_address : address`, `||--o{` for array items); edges are
    /// relationships between their connections' labels.
    ///
    /// Labels and property names are user text: a name Mermaid cannot take as it is gets a safe
    /// identifier (unique in the diagram, or in the entity for an attribute) and keeps its text
    /// (an entity alias `id["text"]`, the attribute comment, a quoted relationship label), so
    /// the output always parses (`mermaid_*` helpers below).
    pub fn to_mermaid(&self) -> String {
        let mut names = MermaidNames::default();
        let mut lines = vec!["erDiagram".to_string()];
        let mut links = Vec::new();
        let mut entities = Vec::new();
        for (label, vertex) in &self.vertices {
            let id = names.label(label);
            entities.push((id, label.clone(), &vertex.schema));
        }
        for (label, edge) in &self.edges {
            if !edge.schema.properties.is_empty() {
                let id = names.unique(&mermaid_identifier(&format!("edge_{label}")));
                entities.push((id, format!("{label} (edge)"), &edge.schema));
            }
        }
        for (id, text, node) in entities {
            mermaid_entity(&id, &text, node, &mut names, &mut lines, &mut links);
        }
        lines.extend(links);
        for (label, edge) in &self.edges {
            for conn in &edge.connections {
                let from = mermaid_endpoint(&mut names, &mut lines, &conn.from);
                let to = mermaid_endpoint(&mut names, &mut lines, &conn.to);
                lines.push(format!(
                    "  {from} ||--o{{ {to} : {}",
                    mermaid_relationship_label(label)
                ));
            }
        }
        lines.join("\n")
    }

    /// PlantUML: the label topology as `entity` diagrams (`@startuml`), followed by the
    /// condensed schema as a `@startjson` view (types and constraints as short strings).
    pub fn to_plantuml(&self) -> String {
        let mut lines = vec![
            "@startuml".to_string(),
            "!pragma layout smetana".to_string(),
        ];
        let entity =
            |lines: &mut Vec<String>, name: &str, stereotype: &str, root: &PropertySchema| {
                lines.push(format!(
                    "entity \"{name}\" as {} << ({stereotype}) >> {{",
                    sanitize_identifier(name)
                ));
                if let Some(description) = root.annotations().and_then(|a| a.description.as_deref())
                {
                    lines.push(format!("  -- {} --", description.replace('\n', " ")));
                }
                let mut rows = Vec::new();
                flatten(root, "", &mut rows);
                for row in rows {
                    let marker = if row.required { "*" } else { " " };
                    let constraints = constraint_text(row.node);
                    let suffix = if constraints.is_empty() {
                        String::new()
                    } else {
                        format!(" [{constraints}]")
                    };
                    let nullable = if row.node.admits_null() { "?" } else { "" };
                    lines.push(format!(
                        "  {marker} {} : {}{nullable}{suffix}",
                        row.path,
                        row.node.non_null_type_name()
                    ));
                }
                lines.push("}".to_string());
            };
        for (label, vertex) in &self.vertices {
            entity(&mut lines, label, "V,#ADD8E6", &vertex.schema);
        }
        for (label, edge) in &self.edges {
            for conn in &edge.connections {
                lines.push(format!(
                    "{} --> {} : \"{}\"",
                    sanitize_identifier(&conn.from),
                    sanitize_identifier(&conn.to),
                    sanitize_label(label)
                ));
            }
        }
        lines.push("@enduml".to_string());
        lines.push(String::new());
        lines.push("@startjson".to_string());
        lines.push(pretty_json(&self.condensed()));
        lines.push("@endjson".to_string());
        lines.join("\n")
    }

    /// The condensed schema of the `@startjson` view: every property is a short string (`"int64,
    /// required, 0..150"`), a nested object is an object of its keys.
    fn condensed(&self) -> ElementProperty {
        let mut root = PropertyMap::default();
        root.insert("mode", ElementProperty::from(self.mode.as_str()));
        let mut vertices = PropertyMap::default();
        for (label, vertex) in &self.vertices {
            vertices.insert(label.as_str(), condensed_object(&vertex.schema));
        }
        root.insert("vertices", ElementProperty::Object(Box::new(vertices)));
        let mut edges = PropertyMap::default();
        for (label, edge) in &self.edges {
            let key = if edge.connections.is_empty() {
                label.clone()
            } else {
                format!(
                    "{label} ({})",
                    edge.connections
                        .iter()
                        .map(|c| format!("{} -> {}", c.from, c.to))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            edges.insert(key, condensed_object(&edge.schema));
        }
        root.insert("edges", ElementProperty::Object(Box::new(edges)));
        ElementProperty::Object(Box::new(root))
    }
}

/// Short text of one property for the condensed view.
fn condensed_text(node: &PropertySchema, required: bool) -> String {
    let mut parts = vec![node.non_null_type_name()];
    if required {
        parts.push("required".to_string());
    }
    if node.admits_null() {
        parts.push("nullable".to_string());
    }
    let constraints = constraint_text(node);
    if !constraints.is_empty() {
        parts.push(constraints);
    }
    parts.join(", ")
}

fn condensed_object(node: &PropertySchema) -> ElementProperty {
    let mut map = PropertyMap::default();
    for (name, child, required, is_items) in children(node) {
        let has_keys = !children(child).is_empty();
        if is_items {
            map.insert("[]", ElementProperty::Array(vec![condensed_object(child)]));
        } else if has_keys {
            let key = format!("{name} ({})", condensed_text(child, required));
            map.insert(key, condensed_object(child));
        } else {
            map.insert(name, ElementProperty::from(condensed_text(child, required)));
        }
    }
    ElementProperty::Object(Box::new(map))
}

/// The identifiers of a Mermaid diagram: one per label (the same label always gets the same
/// one), unique across the diagram (`a-b` and `a_b` are two entities, `a_b` and `a_b_2`).
#[derive(Default)]
struct MermaidNames {
    by_label: HashMap<String, String>,
    used: HashSet<String>,
}

impl MermaidNames {
    /// The identifier of a label's entity.
    fn label(&mut self, label: &str) -> String {
        if let Some(id) = self.by_label.get(label) {
            return id.clone();
        }
        let id = if label == "*" {
            self.unique("any_label")
        } else {
            self.unique(&mermaid_identifier(label))
        };
        self.by_label.insert(label.to_string(), id.clone());
        id
    }

    /// `base`, or `base_2`, `base_3`, ... when it is taken.
    fn unique(&mut self, base: &str) -> String {
        let mut id = base.to_string();
        let mut n = 2;
        while !self.used.insert(id.clone()) {
            id = format!("{base}_{n}");
            n += 1;
        }
        id
    }
}

/// A Mermaid identifier for a name: letters (any script), digits and `_`, every other character
/// `_`, never starting with a digit (`1st` -> `_1st`), never empty.
fn mermaid_identifier(name: &str) -> String {
    let mut id: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if id.is_empty() {
        id.push_str("entity");
    } else if id.starts_with(|c: char| c.is_ascii_digit()) {
        id.insert(0, '_');
    }
    id
}

/// Text inside a Mermaid double-quoted string (an attribute comment, a relationship label): no
/// `"` (Mermaid has no escape for it) and no line breaks; every other character parses there.
fn mermaid_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '"' => '\'',
            '\n' | '\r' | '\t' => ' ',
            c => c,
        })
        .collect()
}

/// The text of an entity alias (`id["text"]`): as [`mermaid_text`], and `%` and `\` (which an
/// alias cannot hold, measured against Mermaid 11) as their full-width forms `％` and `＼`.
fn mermaid_alias_text(text: &str) -> String {
    mermaid_text(text)
        .chars()
        .map(|c| match c {
            '%' => '％',
            '\\' => '＼',
            c => c,
        })
        .collect()
}

/// A relationship label: a plain word as it is (`knows`), anything else quoted.
fn mermaid_relationship_label(label: &str) -> String {
    let plain = !label.is_empty()
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !label.starts_with(|c: char| c.is_ascii_digit());
    if plain {
        label.to_string()
    } else {
        format!("\"{}\"", mermaid_text(label))
    }
}

/// The entity of a connection's endpoint; one declared nowhere else (`*`, a label without a
/// schema of its own) is added as an empty entity so its text shows.
fn mermaid_endpoint(names: &mut MermaidNames, lines: &mut Vec<String>, label: &str) -> String {
    let known = names.by_label.contains_key(label);
    let id = names.label(label);
    if !known {
        let text = if label == "*" { "* (any label)" } else { label };
        lines.push(format!("  {} {{", mermaid_entity_head(&id, text)));
        lines.push("  }".to_string());
    }
    id
}

/// `id`, or `id["text"]` when the identifier differs from the text it stands for.
fn mermaid_entity_head(id: &str, text: &str) -> String {
    if id == text {
        id.to_string()
    } else {
        format!("{id}[\"{}\"]", mermaid_alias_text(text))
    }
}

fn mermaid_entity(
    name: &str,
    text: &str,
    node: &PropertySchema,
    names: &mut MermaidNames,
    lines: &mut Vec<String>,
    links: &mut Vec<String>,
) {
    let mut attributes = Vec::new();
    let mut nested = Vec::new();
    let mut attribute_names = HashSet::new();
    for (key, child, required, is_items) in children(node) {
        if is_items {
            continue;
        }
        let base = mermaid_identifier(&key);
        let mut ident = base.clone();
        let mut n = 2;
        while !attribute_names.insert(ident.clone()) {
            ident = format!("{base}_{n}");
            n += 1;
        }
        let mut notes = Vec::new();
        // A renamed attribute keeps its real name in the comment.
        if ident != key {
            notes.push(format!("{key}:"));
        }
        if required {
            notes.push("required".to_string());
        }
        if child.admits_null() {
            notes.push("nullable".to_string());
        }
        if let Some(dimension) = child.vector.as_ref().and_then(|v| v.dimension) {
            notes.push(format!("dimension {dimension}"));
        }
        let constraints = constraint_text(child);
        if !constraints.is_empty() {
            notes.push(constraints);
        }
        let comment = if notes.is_empty() {
            String::new()
        } else {
            let joined = notes.join(", ").replacen(":,", ":", 1);
            format!(" \"{}\"", mermaid_text(&joined))
        };
        attributes.push(format!("    {} {ident}{comment}", mermaid_type(child)));
        // A nested object, or the object items of an array, becomes an entity of its own.
        let has_keys = |n: &PropertySchema| {
            !n.properties.is_empty()
                || n.any_of
                    .as_ref()
                    .is_some_and(|m| m.iter().any(|m| !m.properties.is_empty()))
        };
        let child_entity = if has_keys(child) {
            Some((child.clone(), "||--||"))
        } else if let Some(items) = &child.items
            && has_keys(items)
        {
            Some(((**items).clone(), "||--o{"))
        } else {
            None
        };
        if let Some((schema, cardinality)) = child_entity {
            let child_name = names.unique(&format!("{name}_{ident}"));
            // Named by its identifier (`person_address`): the link names the key.
            nested.push((
                child_name.clone(),
                child_name,
                schema,
                key.clone(),
                cardinality,
            ));
        }
    }
    lines.push(format!("  {} {{", mermaid_entity_head(name, text)));
    lines.extend(attributes);
    lines.push("  }".to_string());
    for (child_name, child_text, child, key, cardinality) in nested {
        links.push(format!(
            "  {name} {cardinality} {child_name} : {}",
            mermaid_relationship_label(&key)
        ));
        mermaid_entity(&child_name, &child_text, &child, names, lines, links);
    }
}

/// A Mermaid attribute type: letters, digits, `_` and `~` only (array items as `array~T~`, a
/// vector as `vector~i8~`, its dimension in the comment; a union as `union`).
fn mermaid_type(node: &PropertySchema) -> String {
    if let Some(vector) = &node.vector {
        return format!("vector~{}~", vector.element.name());
    }
    let text = node.non_null_type_name();
    if text.starts_with("union<") {
        return "union".to_string();
    }
    text.chars()
        .filter_map(|c| match c {
            '<' | '>' => Some('~'),
            c if c.is_ascii_alphanumeric() || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// A copy of `node` with `additionalProperties` written on every object node that states none.
fn explicit_additional(node: &PropertySchema, default: Option<bool>) -> PropertySchema {
    let mut copy = node.clone();
    fn fill(node: &mut PropertySchema, default: Option<bool>) {
        if node.additional_properties.is_none()
            && node
                .types
                .as_ref()
                .is_some_and(|t| t.contains(&JsonType::Object))
        {
            node.additional_properties = default;
        }
        // A vector is an array of numbers to another validator (which ignores the custom
        // `format`, `element` and `dimension`): spelled out, so it checks the structure too.
        if let Some(vector) = &node.vector {
            node.items = Some(Box::new(PropertySchema::number()));
            if let Some(dimension) = vector.dimension {
                let constraints = node.constraints.get_or_insert_with(Box::default);
                constraints.min_items = Some(u64::from(dimension));
                constraints.max_items = Some(u64::from(dimension));
            }
        }
        for child in node.properties.values_mut() {
            fill(child, default);
        }
        if let Some(items) = &mut node.items {
            fill(items, default);
        }
        if let Some(members) = &mut node.any_of {
            for member in members {
                fill(member, default);
            }
        }
    }
    fill(&mut copy, default);
    copy
}

fn sanitize_identifier(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "Entity".to_string()
    } else {
        sanitized
    }
}

fn sanitize_label(label: &str) -> String {
    label.replace('"', "").replace(':', "_")
}

/// A JSON value of the schema (`enum`, `const`, `default`, `meta`) as compact JSON text.
fn json_text(value: &ElementProperty) -> String {
    crate::value::physical::to_json_text(value).unwrap_or_default()
}
