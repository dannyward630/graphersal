//! The infix connectives `a.and().b` / `a.or().b` (TinkerPop's `ConnectiveStrategy`), folded
//! into `and(a, b)` / `or(a, b)` on the step AST before it is lowered.
//!
//! The rules are TinkerPop's: `or()` is folded first, then `and()` (inside every `or()` branch
//! too), so `and()` binds tighter (`a.and().b.or().c` is `or(and(a, b), c)`). The left operand
//! is every step before the marker back to the start of the traversal (a root's start step,
//! `V()`/`E()`/`inject()`/..., and the step labels it carries stay outside); the right operand
//! is every step after it to the end of the traversal, split at each further marker of the same
//! kind (`a.and().b.and().c` is `and(a, b, c)`).

use super::*;
use std::borrow::Cow;

/// Which infix marker is folded.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Connective {
    And,
    Or,
}

impl Connective {
    fn is_marker(self, step: &Step) -> bool {
        matches!(
            (self, step),
            (Connective::And, Step::AndInfix) | (Connective::Or, Step::OrInfix)
        )
    }
}

impl ScriptGraphTraversalSource {
    /// The steps of this traversal with every infix connective folded (borrowed when there is
    /// none). `root` marks the rooted traversal, whose first step starts it and is never part
    /// of an operand.
    pub(super) fn fold_connectives(&self, root: bool) -> Cow<'_, [Step]> {
        if !self
            .steps
            .iter()
            .any(|step| matches!(step, Step::AndInfix | Step::OrInfix))
        {
            return Cow::Borrowed(&self.steps);
        }
        let steps = self.fold_connective(self.steps.clone(), Connective::Or, root);
        Cow::Owned(self.fold_connective(steps, Connective::And, root))
    }

    /// Folds the first marker of `kind` in `steps`; everything after it becomes part of the
    /// connective, so one marker per level is enough. Markers of the other kind stay where they
    /// are (an `and()` marker inside an `or()` operand is folded when that operand is lowered).
    fn fold_connective(&self, mut steps: Vec<Step>, kind: Connective, root: bool) -> Vec<Step> {
        let Some(at) = steps.iter().position(|step| kind.is_marker(step)) else {
            return steps;
        };
        // The left operand starts after the root's start step (source steps such as
        // `withSack()` come first and are not steps of the stream).
        let mut start = 0;
        if root {
            while start < at && matches!(steps[start], Step::Source(_)) {
                start += 1;
            }
            if start < at {
                start += 1;
            }
        }
        // Step labels directly after the start step label it, not the operand.
        while start < at && matches!(steps[start], Step::As(_)) {
            start += 1;
        }
        let tail = steps.split_off(start);
        let mut operands: Vec<Vec<Step>> = vec![Vec::new()];
        for step in tail {
            if kind.is_marker(&step) {
                operands.push(Vec::new());
            } else if let Some(current) = operands.last_mut() {
                current.push(step);
            }
        }
        let children = operands
            .into_iter()
            .map(|operand| self.connective_child(operand))
            .collect();
        steps.push(match kind {
            Connective::And => Step::And(children),
            Connective::Or => Step::Or(children),
        });
        steps
    }

    /// A child traversal holding `steps` (lowered as `__`, where its own infix connectives are
    /// folded in turn).
    fn connective_child(&self, steps: Vec<Step>) -> ScriptGraphTraversalSource {
        let mut child = self.clone();
        child.steps = steps;
        child
    }
}
