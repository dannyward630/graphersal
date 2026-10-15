//! The write checks and JSONPath write helpers of the storage contract, public through
//! [`crate::storage`].
//!
//! They are the minimum the [`GraphStorage`](crate::storage::GraphStorage) contract requires on
//! writes, with exactly the errors the conformance suite (`graphersal-storage-tests`) expects.
//! [`TraversalGraph`](crate::TraversalGraph) and the trait's default methods call these same
//! functions, so a storage built on them answers like the reference one. A storage may refuse
//! more (its own limits, its own validation) with errors of its own; nothing here forbids that.

use compact_str::CompactString;

use crate::error::GraphError;
use crate::value::json_path::{self, JsonPathSegment};
use crate::value::{ElementProperty, JsonPath, PropertyMap};

/// Rejects a property value that is, or contains at any depth, a vertex, an edge or a map with a
/// non-string key: [`GraphError::InvalidPropertyValue`] naming `key`.
///
/// Graph elements are stream values, not property values, so this holds in every schema mode and
/// runs before any schema validation: call it in `insert_vertex`/`insert_edge` (through
/// [`check_properties`]), `set_*_property` and the path writes (with the path's property name).
///
/// ```
/// use graphersal::storage::{check_property_value, ElementProperty, GraphError};
///
/// assert!(check_property_value("age", &ElementProperty::from(29_i64)).is_ok());
/// ```
pub fn check_property_value(key: &str, value: &ElementProperty) -> Result<(), GraphError> {
    match value.element_ref_kind() {
        Some(kind) => Err(GraphError::invalid_property_value(key, kind)),
        None => Ok(()),
    }
}

/// [`check_property_value`] for every property of an element being inserted.
pub fn check_properties(properties: &PropertyMap) -> Result<(), GraphError> {
    properties
        .iter()
        .try_for_each(|(key, value)| check_property_value(key, value))
}

/// Rejects a vertex label containing the substring `"::"`, which the GraphSON and GraphML
/// multi-label export reserves as the delimiter between labels: [`GraphError::InvalidLabel`].
/// Run it in `insert_vertex` (through [`normalize_vertex_labels`]) and `add_vertex_label`.
pub fn check_label(label: &str) -> Result<(), GraphError> {
    if label.contains("::") {
        Err(GraphError::invalid_label(label))
    } else {
        Ok(())
    }
}

/// The label set `insert_vertex` stores for `labels`: duplicates removed (the first occurrence
/// keeps its position, labels are a set) and every label checked with [`check_label`].
pub fn normalize_vertex_labels(
    labels: Vec<CompactString>,
) -> Result<Vec<CompactString>, GraphError> {
    let mut out: Vec<CompactString> = Vec::with_capacity(labels.len());
    for label in labels {
        check_label(&label)?;
        if !out.contains(&label) {
            out.push(label);
        }
    }
    Ok(out)
}

/// The stored property a write path addresses: its first segment, which must be a name
/// (otherwise [`GraphError::InvalidPropertyPath`] with
/// [`PathWriteFault::FirstSegmentNotName`](crate::error::PathWriteFault::FirstSegmentNotName)).
pub fn path_property_name(path: &JsonPath) -> Result<&CompactString, GraphError> {
    json_path::write_property_name(path)
        .map_err(|fault| GraphError::invalid_property_path(path, fault))
}

/// Applies a path write (`property(jpath(..), value)`) to a copy of the stored top-level property
/// `existing` (`None` when the element does not have it) and returns the property name with the
/// whole resulting value, to be stored like a plain `set_*_property`. This is what the default
/// `GraphStorage::set_vertex_property_path` does.
///
/// The rules: missing object keys are created (only when every remaining segment is a name; an
/// index never creates an array), `[n]` replaces, `[len]` appends (last segment only), a negative
/// index counts from the end, any mismatch is [`GraphError::InvalidPropertyPath`] with its
/// [`PathWriteFault`](crate::error::PathWriteFault). `existing` itself is never changed. The
/// value is not checked: run [`check_property_value`] on it first.
///
/// ```
/// use graphersal::storage::{write_path, ElementProperty, JsonPath};
///
/// let path: JsonPath = "meta.k".parse().unwrap();
/// let (name, value) = write_path(None, &path, ElementProperty::from(1_i64)).unwrap();
/// assert_eq!(name, "meta");
/// assert!(matches!(value, ElementProperty::Object(_)));
/// ```
pub fn write_path(
    existing: Option<&ElementProperty>,
    path: &JsonPath,
    value: ElementProperty,
) -> Result<(CompactString, ElementProperty), GraphError> {
    json_path::write_path_to_copy(existing, path, value)
        .map_err(|fault| GraphError::invalid_property_path(path, fault))
}

/// [`write_path`] in place on an element's property map (the first segment names the property,
/// created when missing). Every fault is found before anything changes, so a refused write leaves
/// `properties` as it was.
pub fn write_path_in(
    properties: &mut PropertyMap,
    path: &JsonPath,
    value: ElementProperty,
) -> Result<(), GraphError> {
    json_path::write_path_in_element(properties, path, value)
        .map_err(|fault| GraphError::invalid_property_path(path, fault))
}

/// Applies a path removal (`remove_property(jpath(..))`, two or more segments) to a copy of the
/// stored top-level property `existing` and returns the property name with the resulting value,
/// or `None` when there is nothing to remove (a missing path, an out-of-range index or a type
/// mismatch on the way is not an error). A one-segment path is a plain property drop and also
/// gives `None` here. What the default `GraphStorage::remove_vertex_property_path` does.
pub fn remove_path(
    existing: Option<&ElementProperty>,
    path: &JsonPath,
) -> Option<(CompactString, ElementProperty)> {
    json_path::remove_path_to_copy(existing, path)
}

/// [`remove_path`] in place on an element's property map; `true` when something was removed.
pub fn remove_path_in(properties: &mut PropertyMap, path: &JsonPath) -> bool {
    json_path::remove_path_in_element(properties, path)
}

/// The property name of a one-segment path (`remove_property(jpath("age"))` = plain removal).
pub(crate) fn single_segment_name(path: &JsonPath) -> Option<&str> {
    match path.segments() {
        [JsonPathSegment::Key(name)] => Some(name.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::PathWriteFault;

    fn path(text: &str) -> JsonPath {
        text.parse().expect("valid path")
    }

    fn ints(values: &[i64]) -> ElementProperty {
        ElementProperty::Array(values.iter().map(|v| ElementProperty::from(*v)).collect())
    }

    fn object(entries: &[(&str, ElementProperty)]) -> ElementProperty {
        let mut map = PropertyMap::default();
        for (key, value) in entries {
            map.insert(CompactString::from(*key), value.clone());
        }
        ElementProperty::Object(Box::new(map))
    }

    fn fault_of(err: GraphError) -> PathWriteFault {
        match err {
            GraphError::InvalidPropertyPath { fault, .. } => fault,
            other => panic!("not a path fault: {other:?}"),
        }
    }

    #[test]
    fn element_references_are_rejected_at_any_depth() {
        let vertex = ElementProperty::Vertex(Box::default());
        let err = check_property_value("friend", &vertex).expect_err("a vertex");
        assert!(matches!(err, GraphError::InvalidPropertyValue { .. }));
        assert_eq!(
            err.to_string(),
            GraphError::invalid_property_value("friend", "vertex").to_string()
        );
        let nested = ElementProperty::Array(vec![ElementProperty::from(1_i64), vertex]);
        assert!(check_property_value("xs", &nested).is_err());
        let mut props = PropertyMap::default();
        props.insert(CompactString::from("ok"), 1_i64.into());
        assert!(check_properties(&props).is_ok());
        props.insert(CompactString::from("bad"), nested);
        assert!(check_properties(&props).is_err());
    }

    #[test]
    fn labels_are_checked_and_deduplicated() {
        assert!(matches!(
            check_label("a::b"),
            Err(GraphError::InvalidLabel { .. })
        ));
        let labels = normalize_vertex_labels(vec!["b".into(), "a".into(), "b".into()]).unwrap();
        assert_eq!(
            labels,
            vec![CompactString::from("b"), CompactString::from("a")]
        );
        assert!(normalize_vertex_labels(vec!["a".into(), "x::y".into()]).is_err());
    }

    #[test]
    fn every_path_write_fault_is_reported() {
        let tags = ints(&[1, 2]);
        let name = ElementProperty::from("ann");
        assert!(matches!(
            fault_of(path_property_name(&path("[0]")).unwrap_err()),
            PathWriteFault::FirstSegmentNotName
        ));
        assert!(matches!(
            fault_of(write_path(None, &path("[0].a"), 1_i64.into()).unwrap_err()),
            PathWriteFault::FirstSegmentNotName
        ));
        assert!(matches!(
            fault_of(write_path(Some(&name), &path("name.x"), 1_i64.into()).unwrap_err()),
            PathWriteFault::KeyOnNonObject {
                found: "string",
                ..
            }
        ));
        assert!(matches!(
            fault_of(write_path(Some(&name), &path("name[0]"), 1_i64.into()).unwrap_err()),
            PathWriteFault::IndexOnNonArray {
                found: "string",
                ..
            }
        ));
        assert!(matches!(
            fault_of(write_path(Some(&tags), &path("tags[7]"), 1_i64.into()).unwrap_err()),
            PathWriteFault::IndexOutOfRange {
                index: 7,
                len: 2,
                ..
            }
        ));
        assert!(matches!(
            fault_of(write_path(None, &path("meta.tags[0]"), 1_i64.into()).unwrap_err()),
            PathWriteFault::MissingArray { .. }
        ));
        let err =
            GraphError::invalid_property_path(&path("a.b"), PathWriteFault::CardinalityWithPath);
        assert!(matches!(fault_of(err), PathWriteFault::CardinalityWithPath));
    }

    #[test]
    fn nested_writes_and_removals_succeed() {
        let tags = ints(&[1, 2]);
        let (key, value) = write_path(Some(&tags), &path("tags[-1]"), 9_i64.into()).unwrap();
        assert_eq!((key.as_str(), value), ("tags", ints(&[1, 9])));
        let (_, value) = write_path(Some(&tags), &path("tags[2]"), 3_i64.into()).unwrap();
        assert_eq!(value, ints(&[1, 2, 3]));
        assert_eq!(tags, ints(&[1, 2]), "the existing value is never changed");

        let mut props = PropertyMap::default();
        write_path_in(&mut props, &path("meta.a.b"), 1_i64.into()).unwrap();
        let b = object(&[("b", 1_i64.into())]);
        assert_eq!(props.get("meta"), Some(&object(&[("a", b)])));
        assert!(write_path_in(&mut props, &path("meta.a.b.c"), 1_i64.into()).is_err());
        let meta = props.get("meta").cloned();
        let (key, removed) = remove_path(meta.as_ref(), &path("meta.a.b")).unwrap();
        assert_eq!(
            (key.as_str(), removed),
            ("meta", object(&[("a", object(&[]))]))
        );
        assert!(remove_path(meta.as_ref(), &path("meta.zzz")).is_none());
        assert!(remove_path(meta.as_ref(), &path("meta")).is_none());
        assert!(remove_path_in(&mut props, &path("meta.a")));
        assert!(!remove_path_in(&mut props, &path("meta.a")));
    }
}
