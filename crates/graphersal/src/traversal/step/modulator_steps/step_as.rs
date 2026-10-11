use std::sync::Arc;

use crate::graph::GraphStorage;
use crate::traversal::optimizer::path_analysis::PathMode;
use crate::*;

#[derive(Default, Clone)]
struct StepAs {
    /// Labels attached to the current path position, shared with the path nodes by refcount.
    labels: Option<Arc<[CompactString]>>,
}

impl StepAs {
    fn new(labels: ArgStrings) -> Self {
        let labels = match labels {
            ArgStrings::None => None,
            ArgStrings::String(v) => Some(Arc::from(vec![CompactString::new(v)])),
            ArgStrings::List(v) => Some(v.into_iter().map(CompactString::new).collect()),
        };
        Self { labels }
    }
}

impl fmt::Display for StepAs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let labels: &[CompactString] = self.labels.as_deref().unwrap_or(&[]);
        write!(f, "as({})", crate::value::literal::rhai_strings(labels))
    }
}

impl<'g, S: GraphStorage + 'g> GraphTraversalSource<'g, S> {
    /// Labels the current step so select(), where() and path() can refer back to it.
    pub fn as_<T: Into<ArgStrings>>(self, labels: T) -> Self {
        self.push_step(GraphStep::As("as", Box::new(StepAs::new(labels.into()))))
    }
}

impl __ {
    /// Labels the current step so select(), where() and path() can refer back to it. Starts an
    /// anonymous traversal (`__.as()`).
    pub fn as_<'g, S: GraphStorage + 'g, T: Into<ArgStrings>>(
        labels: T,
    ) -> AnonymousTraversal<'g, S> {
        AnonymousTraversal::new().as_(labels)
    }
}

impl<'g, S: GraphStorage + 'g> AnonymousTraversal<'g, S> {
    /// Labels the current step so select(), where() and path() can refer back to it.
    pub fn as_<T: Into<ArgStrings>>(self, labels: T) -> Self {
        let step = GraphStep::As("as", Box::new(StepAs::new(labels.into())));
        self.push_step(step)
    }
}

impl<'g, S: GraphStorage + 'g> GraphStepAs<'g, S> for StepAs {
    fn process(
        &self,
        context: &mut TraverserContext<'_, S>,
        source: &mut Traverser<S>,
    ) -> TraverserResult<()> {
        let Some(labels) = &self.labels else {
            source.keep_path(&context.path_mode);
            return Ok(());
        };
        match &context.path_mode {
            // The current object already is the last position: label it.
            PathMode::Full => {
                source.path = context
                    .paths
                    .relabel_last(source.path, &source.value, labels)?;
            }
            // Only labelled positions are recorded, and only when a later step reads one of them.
            PathMode::Labels(read) => {
                if read.intersects(labels) {
                    source.path = context.paths.push(
                        source.path,
                        source.value.clone(),
                        Some(labels.clone()),
                    )?;
                }
            }
            PathMode::None => source.path = PathRef::NONE,
        }
        Ok(())
    }

    fn step_labels(&self) -> Option<&Arc<[CompactString]>> {
        self.labels.as_ref()
    }

    fn clone_box(&self) -> Box<dyn GraphStepAs<'g, S> + 'g> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .v("1")
            .as_("A")
            .out(None)
            .select("A")
            .id()
            .dedup()
            .to_list()
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_string(), Some("1"));
    }

    #[test]
    fn where_t() {
        let graph = GraphSource::tinkerpop_modern();
        let g_lock = graph.read();
        let g = g_lock.traversal();
        let result = g
            .v(None)
            .as_("A")
            .where_t(__::as_("A").out(None).select("A"))
            .id()
            .dedup()
            .to_list()
            .unwrap();
        assert_eq!(result.len(), 3);
    }
}
