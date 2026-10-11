//! Maps with non-string keys as the script sees them (`ScriptMap`).

use crate::script::*;

/// A map with at least one non-string key as the script sees it (`{29: 1}`, `{["josh"]: 1}`): the
/// [`crate::value::ValueMap`] of an [`ElementProperty::Map`], carried natively.
///
/// A map whose keys are all strings stays a Rhai object map (`rhai::Map`); only a map that
/// cannot be one (a `group().by("age")` result, say) is a `ScriptMap`. It supports `m[29]` /
/// `m["name"]` (a missing key yields `()`), `m.name` for a string key, `len()`, `is_empty()`,
/// `keys()`, `values()`, `contains(k)` / `containsKey(k)`, `for pair in m` (`[key, value]` arrays),
/// `to_map()` (a `rhai::Map`, an error when a key is not a string) and prints as `{29: 1, "a": 2}`.
/// Registered with Rhai as `Map`. Public so hosts and tests can downcast it, like [`ScriptUuid`].
#[derive(Clone, PartialEq)]
pub struct ScriptMap(pub(crate) crate::value::ValueMap);

impl ScriptMap {
    /// The wrapped map.
    pub fn as_value_map(&self) -> &crate::value::ValueMap {
        &self.0
    }
}

impl IntoIterator for ScriptMap {
    type Item = Dynamic;
    type IntoIter = std::vec::IntoIter<Dynamic>;

    fn into_iter(self) -> Self::IntoIter {
        self.0
            .into_iter()
            .map(|(key, value)| {
                Dynamic::from_array(vec![
                    element_property_to_dynamic(key),
                    element_property_to_dynamic(value),
                ])
            })
            .collect::<Vec<Dynamic>>()
            .into_iter()
    }
}

/// Renders a map key for the console form `{29: 1, "a": 2}`: a string quoted (JSON), anything else
/// through the display renderer.
pub(crate) fn map_key_text(key: &ElementProperty) -> String {
    match key {
        ElementProperty::String(_) => crate::value::physical::to_json_text(key).unwrap_or_default(),
        other => crate::value::physical::to_display_text(other).to_string(),
    }
}

/// Renders a [`ScriptMap`] as `{29: 1, "a": 2}`, cutting the entries to the item limit.
pub(crate) fn script_map_text(
    map: &ScriptMap,
    limit: &crate::traversal::render::ItemLimit,
) -> String {
    let keep = limit.keep(map.0.len());
    let mut entries: Vec<String> = map
        .0
        .iter()
        .take(keep)
        .map(|(key, value)| {
            format!(
                "{}: {}",
                map_key_text(key),
                bounded_debug_value(&element_property_to_dynamic(value.clone()), limit)
            )
        })
        .collect();
    if keep < map.0.len() {
        entries.push(crate::traversal::render::CUT_MARKER.to_string());
    }
    format!("{{{}}}", entries.join(", "))
}

/// `ScriptMap::to_map()`: the entries as a Rhai object map, or an error when a key is not a string.
pub(crate) fn script_map_to_rhai_map(
    map: &ScriptMap,
) -> Result<rhai::Map, Box<rhai::EvalAltResult>> {
    let mut out = rhai::Map::new();
    for (key, value) in map.0.iter() {
        match key {
            ElementProperty::String(name) => {
                out.insert(
                    name.as_str().into(),
                    element_property_to_dynamic(value.clone()),
                );
            }
            other => {
                return Err(script_error_from_script(
                    crate::error::ScriptError::ArgumentMismatch {
                        step_name: "to_map".to_string(),
                        expected: "a map whose keys are all strings".to_string(),
                        actual: format!("a map with the non-string key {}", map_key_text(other)),
                    },
                ));
            }
        }
    }
    Ok(out)
}

/// Wraps a [`crate::error::ScriptError`] into a script runtime error carrying the full diagnostic report.
pub(crate) fn script_error_from_script(err: crate::error::ScriptError) -> Box<rhai::EvalAltResult> {
    Box::new(rhai::EvalAltResult::ErrorRuntime(
        Dynamic::from(crate::error::format_diagnostic(&err)),
        rhai::Position::NONE,
    ))
}
