use crate::error::TraverserResult;
use crate::traversal::render::{CUT_MARKER, ItemLimit};
use crate::traversal::table::format_cell;
use std::borrow::Cow;

use crate::value::ElementProperty;
use crate::value::physical::to_display_text;

/// Formats traversal elements as an indented hierarchical tree (Unix `tree`-style box drawing).
///
/// This is a generic nested-structure renderer, not one tied to the `tree()` step specifically:
/// any [`ElementProperty::Object`] is treated as a labeled subtree at any depth — its keys become
/// branch labels, its values become children, recursively — so `group()`, `valueMap()`,
/// `elementMap()`, and `select()` results (all nested objects) render sensibly too. An empty
/// `Object` is a leaf with no further children (this is exactly the shape `tree()` itself uses to
/// mark "nothing reached from here" — see `step_tree.rs`), so it prints as a bare leaf, not
/// `label: {}`.
///
/// A result that is *not* nested at all — a plain `g.V()` list, or any other flat stream — is not
/// an error case: it degrades to one root line per element, the same "trivial tree" a single
/// non-branching path already is. There is nothing to reject here; every shape this crate can
/// produce renders as *some* tree, from a bare forest of leaves to a deep hierarchy.
///
/// The caller has already cut the top-level results to `max_rows`. Every nested collection (the
/// entries of an object at any depth, including the unwrapped single root object, and arrays
/// inside leaves) is cut to `limit`, with a `…` line or item marking the cut.
pub(crate) fn format_elements_as_tree(
    elements: &[ElementProperty],
    limit: &ItemLimit,
) -> TraverserResult<String> {
    if elements.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();
    let (roots, cut) = flatten_roots(elements, limit);
    for (key, value) in roots {
        render_node(key.as_deref(), value, "", "", limit, &mut out);
    }
    if cut {
        out.push_str(CUT_MARKER);
        out.push('\n');
    }
    Ok(out.trim_end().to_string())
}

/// Picks the top-level root entries. A single non-empty `Object` result (the shape `tree()`
/// itself always returns — see `step_tree.rs`'s `StepTree::process`) unwraps one level, so its
/// keys become the roots directly instead of sitting under one synthetic wrapper line.
///
/// Anything else — zero, or more than one, or a non-`Object` result — treats each element as its
/// own root. A non-`Object` root stays keyless (its collapsed [`format_table_cell_value`] text
/// *is* its one-line content, e.g. a plain `g.V()` list). A non-empty `Object` root instead gets
/// an index key (`[0]`, `[1]`, ...) rather than its collapsed text — e.g. several `valueMap()`
/// results — because giving it `None` would print the same content twice: once collapsed as the
/// header (`push_header`'s `None` branch) and again expanded as children.
///
/// Also returns whether the unwrapped root object lost entries to `limit`.
fn flatten_roots<'e>(
    elements: &'e [ElementProperty],
    limit: &ItemLimit,
) -> (Vec<(Option<String>, &'e ElementProperty)>, bool) {
    if let [ElementProperty::Object(obj)] = elements
        && !obj.is_empty()
    {
        let keep = limit.keep(obj.len());
        let roots = obj
            .iter()
            .take(keep)
            .map(|(k, v)| (Some(k.to_string()), v))
            .collect();
        return (roots, keep < obj.len());
    }
    // A map with a non-string key unwraps the same way; its keys are shown through the display renderer.
    if let [ElementProperty::Map(map)] = elements {
        let keep = limit.keep(map.len());
        let roots = map
            .iter()
            .take(keep)
            .map(|(k, v)| (Some(to_display_text(k).to_string()), v))
            .collect();
        return (roots, keep < map.len());
    }
    let roots = elements
        .iter()
        .enumerate()
        .map(|(i, e)| match e {
            ElementProperty::Object(obj) if !obj.is_empty() => (Some(format!("[{i}]")), e),
            ElementProperty::Map(_) => (Some(format!("[{i}]")), e),
            _ => (None, e),
        })
        .collect();
    (roots, false)
}

/// Renders one node's own line (its `key`, or — for a keyless root — `value` collapsed via
/// [`format_table_cell_value`]) at `header_prefix`, then, if `value` is a non-empty `Object`,
/// recurses into its entries at `child_prefix`. Arrays and scalars stay collapsed leaves, so a
/// key with a scalar value renders as `key: value` on one line rather than always forcing a
/// separate child line.
fn render_node(
    key: Option<&str>,
    value: &ElementProperty,
    header_prefix: &str,
    child_prefix: &str,
    limit: &ItemLimit,
    out: &mut String,
) {
    match value {
        ElementProperty::Object(obj) if !obj.is_empty() => {
            push_header(key, value, header_prefix, limit, out);
            render_children(
                obj.len(),
                obj.iter().map(|(k, v)| (Cow::Borrowed(k.as_str()), v)),
                child_prefix,
                limit,
                out,
            );
        }
        ElementProperty::Map(map) => {
            push_header(key, value, header_prefix, limit, out);
            let entries = map
                .iter()
                .map(|(k, v)| (Cow::Owned(to_display_text(k).to_string()), v));
            render_children(map.len(), entries, child_prefix, limit, out);
        }
        // An empty Object is a leaf (tree()'s own "no further children" marker) — the key alone,
        // no `: {}` suffix.
        ElementProperty::Object(_) => push_header(key, value, header_prefix, limit, out),
        other => {
            out.push_str(header_prefix);
            match key {
                Some(k) => {
                    out.push_str(k);
                    out.push_str(": ");
                    out.push_str(&format_cell(other, limit));
                }
                None => out.push_str(&format_cell(other, limit)),
            }
            out.push('\n');
        }
    }
}

fn push_header(
    key: Option<&str>,
    value: &ElementProperty,
    header_prefix: &str,
    limit: &ItemLimit,
    out: &mut String,
) {
    out.push_str(header_prefix);
    match key {
        Some(k) => out.push_str(k),
        None => out.push_str(&format_cell(value, limit)),
    }
    out.push('\n');
}

/// Renders `obj`'s entries as connector-prefixed children under `prefix`, at most `limit` of
/// them; a cut ends with a `└─ …` line.
fn render_children<'a>(
    len: usize,
    obj: impl Iterator<Item = (Cow<'a, str>, &'a ElementProperty)>,
    prefix: &str,
    limit: &ItemLimit,
    out: &mut String,
) {
    let keep = limit.keep(len);
    let cut = keep < len;
    let entries: Vec<_> = obj.take(keep).collect();
    let last_idx = entries.len().saturating_sub(1);

    for (i, (key, value)) in entries.into_iter().enumerate() {
        let is_last = i == last_idx && !cut;
        let header_prefix = format!("{prefix}{}", if is_last { "└─ " } else { "├─ " });
        let child_prefix = format!("{prefix}{}", if is_last { "   " } else { "│  " });
        render_node(
            Some(key.as_ref()),
            value,
            &header_prefix,
            &child_prefix,
            limit,
            out,
        );
    }
    if cut {
        out.push_str(prefix);
        out.push_str("└─ ");
        out.push_str(CUT_MARKER);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::PropertyMap;
    use compact_str::CompactString;

    fn obj(pairs: Vec<(&str, ElementProperty)>) -> ElementProperty {
        let mut map = PropertyMap::default();
        for (k, v) in pairs {
            map.insert(CompactString::new(k), v);
        }
        ElementProperty::Object(Box::new(map))
    }

    fn leaf() -> ElementProperty {
        obj(vec![])
    }

    #[test]
    fn single_root_tree_shape() {
        // { "v[1]": { "v[4]": { "v[5]": {}, "v[3]": {} } } }
        let tree = obj(vec![(
            "v[1]",
            obj(vec![(
                "v[4]",
                obj(vec![("v[5]", leaf()), ("v[3]", leaf())]),
            )]),
        )]);
        let rendered =
            format_elements_as_tree(std::slice::from_ref(&tree), &ItemLimit::unlimited()).unwrap();
        assert_eq!(rendered, "v[1]\n└─ v[4]\n   ├─ v[5]\n   └─ v[3]");
    }

    #[test]
    fn flat_non_nested_list_degrades_to_root_lines() {
        let elements = vec![
            ElementProperty::String("a".into()),
            ElementProperty::String("b".into()),
        ];
        let rendered = format_elements_as_tree(&elements, &ItemLimit::unlimited()).unwrap();
        assert_eq!(rendered, "a\nb");
    }

    #[test]
    fn nested_object_with_scalar_leaves() {
        let el = obj(vec![
            ("name", ElementProperty::String("marko".into())),
            ("age", ElementProperty::Number(29.into())),
        ]);
        let rendered =
            format_elements_as_tree(std::slice::from_ref(&el), &ItemLimit::unlimited()).unwrap();
        assert_eq!(rendered, "name: marko\nage: 29");
    }

    #[test]
    fn three_level_branching_merge_shape() {
        // Proves recursion depth > 2 and sibling ordering both work, not just the 3-level fixture.
        let tree = obj(vec![(
            "root",
            obj(vec![("a", obj(vec![("a1", leaf())])), ("b", leaf())]),
        )]);
        let rendered =
            format_elements_as_tree(std::slice::from_ref(&tree), &ItemLimit::unlimited()).unwrap();
        assert_eq!(rendered, "root\n├─ a\n│  └─ a1\n└─ b");
    }

    #[test]
    fn multiple_object_roots_get_index_keys_not_duplicated_content() {
        // Regression: several Object-shaped results (e.g. valueMap() over multiple vertices)
        // must not print their collapsed `{k: v, ...}` text AND expand as children — that
        // duplicated every field. Each gets an index key instead.
        let elements = vec![
            obj(vec![("name", ElementProperty::String("marko".into()))]),
            obj(vec![("name", ElementProperty::String("vadas".into()))]),
        ];
        let rendered = format_elements_as_tree(&elements, &ItemLimit::unlimited()).unwrap();
        assert_eq!(rendered, "[0]\n└─ name: marko\n[1]\n└─ name: vadas");
    }

    #[test]
    fn nested_entries_are_cut_with_a_marker_line() {
        let tree = obj(vec![(
            "root",
            obj(vec![("a", leaf()), ("b", leaf()), ("c", leaf())]),
        )]);
        let limit = ItemLimit::new(Some(2));
        let rendered = format_elements_as_tree(std::slice::from_ref(&tree), &limit).unwrap();
        assert_eq!(rendered, "root\n├─ a\n├─ b\n└─ …");
        assert!(limit.was_cut());
    }

    #[test]
    fn empty_input() {
        assert_eq!(
            format_elements_as_tree(&[], &ItemLimit::unlimited()).unwrap(),
            ""
        );
    }
}
