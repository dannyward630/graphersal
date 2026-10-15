//! `vector_mean()`: the component-wise mean of a stream of vectors (a Graphersal extension, book
//! page "Vectors").

use crate::graph::GraphStorage;
use crate::traversal::step::map_steps::temporal::{describe, held_value};
use crate::value::VectorError;
use crate::value::vector::similarity::MeanAccumulator;
use crate::value::vector::vector_error;
use crate::*;

#[derive(Default, Clone)]
struct StepVectorMean;

impl fmt::Display for StepVectorMean {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("vector_mean()")
    }
}

fn not_a_vector(found: String) -> Box<TraverserError> {
    vector_error("vector_mean", VectorError::NotAVector { found })
}

/// Adds one incoming value: a vector, a list of vectors (each counted), `null` (skipped).
fn add_value<S: GraphStorage>(
    context: &TraverserContext<'_, S>,
    mean: &mut MeanAccumulator,
    value: &TraverserValue<S>,
    bulk: u64,
) -> TraverserResult<()> {
    let add_property = |mean: &mut MeanAccumulator, property: &ElementProperty| match property {
        ElementProperty::Null => Ok(()),
        ElementProperty::Vector(vector) => mean
            .add(vector, bulk)
            .map_err(|error| vector_error("vector_mean", error)),
        other => Err(not_a_vector(describe(other))),
    };
    match value {
        TraverserValue::Array(items) => {
            for item in items.iter() {
                match held_value(context, item)? {
                    Ok(held) => add_property(mean, held.as_ref())?,
                    Err(kind) => return Err(not_a_vector(format!("a {kind}"))),
                }
            }
            Ok(())
        }
        _ => match held_value(context, value)? {
            Ok(held) => match held.as_ref() {
                ElementProperty::Array(items) => {
                    items.iter().try_for_each(|item| add_property(mean, item))
                }
                other => add_property(mean, other),
            },
            Err(kind) => Err(not_a_vector(format!("a {kind}"))),
        },
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepBarrier<'g, S> for StepVectorMean {
    fn materializes(&self) -> bool {
        true
    }

    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: Vec<Traverser<S>>,
    ) -> TraverserResult<Option<Traverser<S>>> {
        let mut mean = MeanAccumulator::new();
        for traverser in &source {
            context.tick()?;
            add_value(context, &mut mean, &traverser.value, traverser.bulk)?;
        }
        // Like mean(): nothing on an empty (or all-null) stream.
        let vector = mean
            .finish()
            .map_err(|error| vector_error("vector_mean", error))?;
        Ok(vector.map(|vector| {
            Traverser::start(
                context,
                TraverserValue::Computed(ElementProperty::Vector(vector), None),
            )
        }))
    }

    fn clone_box(&self) -> Box<dyn GraphStepBarrier<'g, S> + 'g> {
        Box::new(self.clone())
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// `vector_mean()`: the component-wise mean of the incoming vectors as an f32 vector (a
    /// reducing barrier, like `mean()`). An incoming list of vectors (`fold()`) counts each of
    /// its vectors; `null` is skipped; quantized vectors count with their dequantized values;
    /// vectors of different dimensions, or a value that is not a vector, fail. An empty stream
    /// gives no result. A Graphersal extension (book page "Vectors").
    ///
    /// ```rust
    /// use graphersal::prelude::*;
    ///
    /// let graph = GraphSource::empty();
    /// let lock = graph.read();
    /// let a = ElementProperty::Vector(Vector::from_f32(vec![1.0, 0.0]).unwrap());
    /// let b = ElementProperty::Vector(Vector::from_f32(vec![0.0, 1.0]).unwrap());
    /// let mean = lock.traversal().inject(vec![a, b]).vector_mean().next().unwrap().unwrap();
    /// assert_eq!(mean.as_vector().unwrap().to_f32_vec(), vec![0.5, 0.5]);
    /// ```
    pub fn vector_mean(self) -> Self {
        self.push_step(GraphStep::Barrier("vector_mean", Box::new(StepVectorMean)))
    }
}

impl __ {
    /// Anonymous `vector_mean()`; see [`GraphTraversalSource::vector_mean`].
    pub fn vector_mean<'g, S: GraphStorage + 'g>() -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().vector_mean()
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// `vector_mean()`; see [`GraphTraversalSource::vector_mean`].
    pub fn vector_mean(self) -> Self {
        self.push_step(GraphStep::Barrier("vector_mean", Box::new(StepVectorMean)))
    }
}
