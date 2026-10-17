use crate::graph::GraphStorage;
use crate::*;

/// `has(key, traversal)` (TinkerPop's `has(String, Traversal)`): keeps a traverser when the
/// child traversal, run on the value of its property `key`, yields at least one result; an
/// element without the property is filtered out. Exactly TinkerPop's expansion
/// `filter(values(key).traversal)`: the step holds ONE child, `values(key)` followed by the
/// steps of `traversal`, so step ids, paths, authorization and profiles are those of that
/// filter (the child's first step, id `N.0.0`, is the `values(key)`). It renders as written:
/// `has("name", __.to_lower().is("alice"))`.
///
/// The `property_index` rule serves the shape `has(key, __.to_lower().is(P))` (book page
/// "Property Indexes", section "Case-insensitive search") from an index with the transform
/// `to_lower` ([`GraphStepFilter::index_condition`]).
struct StepHasTraversal<'g, S: GraphStorage + 'g> {
    key: PropKey,
    /// `values(key)` followed by the steps of the user's traversal.
    traversal: AnonymousTraversal<'g, S>,
}

impl<'g, S: GraphStorage + 'g> Clone for StepHasTraversal<'g, S> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            traversal: self.traversal.clone(),
        }
    }
}

impl<'g, S: GraphStorage + 'g> fmt::Display for StepHasTraversal<'g, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.key {
            PropKey::Name(name) => {
                write!(f, "has({}, __", crate::value::literal::rhai_string(name))?
            }
            PropKey::Path(path) => {
                write!(f, "has({}, __", crate::value::literal::rhai_jpath(path))?
            }
        }
        // The user's steps: every step after the leading `values(key)`, modulators left out
        // like in every child rendering.
        let mut any = false;
        for step in self.traversal.steps.iter().skip(1) {
            if matches!(step, GraphStep::Modulator(_, _)) {
                continue;
            }
            any = true;
            write!(f, ".{}", step.get_name())?;
        }
        if !any {
            f.write_str(".identity()")?;
        }
        f.write_str(")")
    }
}

/// The step of `has(key, traversal)`.
fn has_traversal_step<'g, S: GraphStorage + 'g>(
    key: PropKey,
    traversal: AnonymousTraversal<'g, S>,
) -> GraphStep<'g, S> {
    let mut child = AnonymousTraversal::new().values(key.clone());
    child.steps.extend(traversal.steps);
    GraphStep::Filter(
        "has_traversal",
        Box::new(StepHasTraversal {
            key,
            traversal: child,
        }),
    )
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// `has(key, traversal)`: keeps the traversers whose property `key` (a name or a
    /// [`JsonPath`]) has a value for which `traversal` yields at least one result, i.e.
    /// `filter(values(key).traversal)` (TinkerPop's `has(String, Traversal)`). An element
    /// without the property is filtered out. `has("name", __.to_lower().is("marko"))` is a
    /// case-insensitive search, served by a property index with the transform `to_lower`.
    pub fn has_traversal<K: Into<PropKey>>(
        self,
        key: K,
        traversal: AnonymousTraversal<'g, S>,
    ) -> Self {
        self.push_step(has_traversal_step(key.into(), traversal))
    }
}

impl __ {
    /// Anonymous `has(key, traversal)`, see [`GraphTraversalSource::has_traversal`].
    pub fn has_traversal<'g, S: GraphStorage + 'g, K: Into<PropKey>>(
        key: K,
        traversal: AnonymousTraversal<'g, S>,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().has_traversal(key, traversal)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// `has(key, traversal)` on an anonymous traversal, see
    /// [`GraphTraversalSource::has_traversal`].
    pub fn has_traversal<K: Into<PropKey>>(
        self,
        key: K,
        traversal: AnonymousTraversal<'g, S>,
    ) -> Self {
        self.push_step(has_traversal_step(key.into(), traversal))
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepFilter<'g, S> for StepHasTraversal<'g, S> {
    /// `has(key, __.to_lower().is(P))` on a literal name, `P` an equality, `P.within` or a
    /// range: the condition a `to_lower` index serves (the constants of `P` as given, compared
    /// with the lowered values). Any other child is not served.
    fn index_condition(&self) -> Option<crate::traversal::step::IndexCondition<'_>> {
        let key = self.key.as_name()?;
        let [values, transform, predicate] = self.traversal.steps.as_slice() else {
            return None;
        };
        let (
            GraphStep::FlatMap("values", _),
            GraphStep::Map(_, transform),
            GraphStep::Filter("is", predicate),
        ) = (values, transform, predicate)
        else {
            return None;
        };
        let transform = transform.index_transform()?;
        let (ranges, reads_path_labels) = predicate.value_index_ranges()?;
        Some(crate::traversal::step::IndexCondition {
            key,
            transform: Some(transform),
            ranges,
            reads_path_labels,
        })
    }

    #[inline]
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: &Traverser<S>,
    ) -> TraverserResult<bool> {
        self.traversal.exists(context, source)
    }

    fn child_traversals_mut(&mut self) -> Vec<&mut crate::traversal::AnonymousTraversal<'g, S>> {
        vec![&mut self.traversal]
    }

    fn analyze_nested_paths(
        &mut self,
        analysis: &crate::traversal::optimizer::path_analysis::PathAnalysis,
        _downstream: &crate::traversal::optimizer::path_analysis::PathRequirement,
    ) -> crate::traversal::optimizer::path_analysis::PathRequirement {
        // Existence test only: the child's output does not continue the path.
        self.traversal.analyze_paths(
            analysis,
            crate::traversal::optimizer::path_analysis::PathRequirement::None,
        )
    }

    fn clone_box(&self) -> Box<dyn GraphStepFilter<'g, S> + 'g> {
        Box::new(self.clone())
    }
}
