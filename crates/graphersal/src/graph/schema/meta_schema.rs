//! Embedded JSON Schema (Draft 2020-12) meta-schema of the Graphersal schema format.

/// The JSON Schema (Draft 2020-12) of the schema format whose `$schema` is
/// [`SCHEMA_FORMAT_URL`](crate::graph::schema::SCHEMA_FORMAT_URL) (the file
/// `graphersal-schema-v1.json`). Editors can use it for completion; the engine's own loader
/// is stricter (it also checks the keyword applicability, `$ref` targets and patterns).
pub const META_SCHEMA_JSON: &str = include_str!("graphersal-schema-v1.json");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_schema_is_valid_json() {
        let parsed = crate::value::physical::read_json(META_SCHEMA_JSON, 64)
            .expect("META_SCHEMA_JSON must be valid JSON");
        let crate::value::physical::PhysicalValue::Object(fields) = parsed else {
            panic!("the meta-schema is an object");
        };
        let id = fields
            .iter()
            .find(|(k, _)| k == "$id")
            .map(|(_, v)| v.clone());
        assert_eq!(
            id,
            Some(crate::value::physical::PhysicalValue::String(
                crate::graph::schema::SCHEMA_FORMAT_URL.into()
            ))
        );
    }
}
