//! `as_vector()` and `quantize(kind)`: the vector conversion steps (Graphersal extensions, book
//! page "Vectors").

use std::borrow::Cow;

use crate::graph::GraphStorage;
use crate::traversal::step::map_steps::temporal::{describe, held_value, replaced};
use crate::value::vector::vector_error;
use crate::value::{Vector, VectorElement, VectorError};
use crate::*;

/// The property value a traverser holds, a list (`fold()`, a projected list) materialized; the
/// stream kind for anything that is no property value.
fn incoming<'a, S: GraphStorage>(
    context: &'a TraverserContext<'_, S>,
    value: &'a TraverserValue<S>,
) -> TraverserResult<Result<Cow<'a, ElementProperty>, &'static str>> {
    match value {
        TraverserValue::Array(_) => Ok(Ok(Cow::Owned(value.into_node_property(context.storage)?))),
        _ => held_value(context, value),
    }
}

/// The error of an input that is not a vector.
fn not_a_vector(step: &'static str, found: String) -> Box<TraverserError> {
    vector_error(step, VectorError::NotAVector { found })
}

/// `as_vector()`: a list of numbers becomes an f32 vector (each number rounded to the nearest
/// f32), a vector passes unchanged (any element kind), `null` stays `null`; anything else, an
/// empty list, a list with a non-number or a non-finite number fails.
#[derive(Clone, Default)]
struct StepAsVector;

impl fmt::Display for StepAsVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("as_vector()")
    }
}

fn as_vector_step<'g, S: GraphStorage + 'g>() -> GraphStep<'g, S> {
    GraphStep::Map("as_vector", Box::new(StepAsVector))
}

impl<'g, S: GraphStorage + 'g> GraphStepMap<'g, S> for StepAsVector {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Traverser<S>> {
        let converted = match incoming(context, &source.value)? {
            Ok(held) => match held.as_ref() {
                ElementProperty::Null => ElementProperty::Null,
                ElementProperty::Vector(vector) => ElementProperty::Vector(vector.clone()),
                ElementProperty::Array(items) => ElementProperty::Vector(
                    Vector::from_values(items).map_err(|error| vector_error("as_vector", error))?,
                ),
                other => return Err(not_a_vector("as_vector", describe(other))),
            },
            Err(kind) => return Err(not_a_vector("as_vector", format!("a {kind}"))),
        };
        Ok(replaced(source, TraverserValue::Computed(converted, None)))
    }

    fn may_filter(&self) -> bool {
        true
    }

    fn clone_box(&self) -> Box<dyn GraphStepMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}

/// `quantize(kind)`: a vector converted to the element kind `kind` ([`Vector::quantize`]: f32
/// to f16 rounds to nearest even, to i8 is symmetric scalar quantization, to bit keeps the sign;
/// `quantize("f32")` dequantizes), `null` stays `null`; anything else fails.
#[derive(Clone)]
struct StepQuantize {
    element: VectorElement,
}

impl fmt::Display for StepQuantize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "quantize({})",
            crate::value::literal::rhai_string(self.element.name())
        )
    }
}

fn quantize_step<'g, S: GraphStorage + 'g>(element: VectorElement) -> GraphStep<'g, S> {
    GraphStep::Map("quantize", Box::new(StepQuantize { element }))
}

impl<'g, S: GraphStorage + 'g> GraphStepMap<'g, S> for StepQuantize {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Traverser<S>,
    ) -> TraverserResult<Traverser<S>> {
        let converted = match incoming(context, &source.value)? {
            Ok(held) => match held.as_ref() {
                ElementProperty::Null => ElementProperty::Null,
                ElementProperty::Vector(vector) => ElementProperty::Vector(
                    vector
                        .quantize(self.element)
                        .map_err(|error| vector_error("quantize", error))?,
                ),
                other => return Err(not_a_vector("quantize", describe(other))),
            },
            Err(kind) => return Err(not_a_vector("quantize", format!("a {kind}"))),
        };
        Ok(replaced(source, TraverserValue::Computed(converted, None)))
    }

    fn may_filter(&self) -> bool {
        true
    }

    fn clone_box(&self) -> Box<dyn GraphStepMap<'g, S> + 'g> {
        Box::new(self.clone())
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// `as_vector()`: converts a list of numbers to an f32 vector; a vector passes, `null` stays
    /// `null`, anything else fails. A Graphersal extension (book page "Vectors").
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let graph = GraphSource::empty();
    /// let lock = graph.read();
    /// let list = ElementProperty::Array(vec![0.5.into(), 1i64.into()]);
    /// let value = lock.traversal().inject(vec![list]).as_vector().next().unwrap().unwrap();
    /// assert_eq!(value.as_vector().unwrap().to_f32_vec(), vec![0.5, 1.0]);
    /// ```
    pub fn as_vector(self) -> Self {
        self.push_step(as_vector_step())
    }

    /// `quantize(kind)`: converts a vector to the element kind `element` (f32, f16, i8, bit; see
    /// [`Vector::quantize`]). A Graphersal extension (book page "Vectors").
    pub fn quantize(self, element: VectorElement) -> Self {
        self.push_step(quantize_step(element))
    }
}

impl __ {
    /// Anonymous `as_vector()`.
    pub fn as_vector<'g, S: GraphStorage + 'g>() -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().as_vector()
    }

    /// Anonymous `quantize(kind)`.
    pub fn quantize<'g, S: GraphStorage + 'g>(element: VectorElement) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().quantize(element)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// `as_vector()`; see [`GraphTraversalSource::as_vector`].
    pub fn as_vector(self) -> Self {
        self.push_step(as_vector_step())
    }

    /// `quantize(kind)`; see [`GraphTraversalSource::quantize`].
    pub fn quantize(self, element: VectorElement) -> Self {
        self.push_step(quantize_step(element))
    }
}
