pub(crate) mod add_property_fold;
pub(crate) mod barrier_pushdown;
pub(crate) mod count_pushdown;
pub(crate) mod filter_pushdown;
pub(crate) mod filter_reorder;
pub(crate) mod group_count_pushdown;
pub(crate) mod has_id_pushdown;
pub(crate) mod lazy_barrier;
pub(crate) mod where_unnest;

/// The distinct entries of a `has_id(..)`/`has_label(..)` argument list, first occurrence first.
///
/// A filter's argument list is a set (`has_id("1", "1")` passes vertex 1 once), but a source
/// step's id/label list is a sequence (`g.V("1", "1")` yields vertex 1 twice, as in TinkerPop), so
/// folding a filter's list into a source step must drop the repeats first.
pub(crate) fn distinct(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::with_capacity(values.len());
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}
