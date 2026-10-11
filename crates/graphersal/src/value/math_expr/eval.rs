//! Evaluator of the compiled postfix program: a loop over [`Instr`] with a value stack that lives
//! on the native stack for every realistic expression (no allocation per evaluation).

use super::{Instr, MathEvalError, MathEvalErrorKind, MathExpr, VarRef};

/// Value-stack size kept on the native stack; deeper programs (pathological nesting) fall back
/// to one heap buffer per evaluation.
const INLINE_STACK: usize = 32;

impl MathExpr {
    /// Evaluates the expression. `resolve` supplies the value of each variable occurrence (its
    /// slot and nested-access path); it is called once per occurrence, in source order.
    /// Resolver errors pass through unchanged; evaluation errors (division by zero) convert
    /// into `E`.
    pub(crate) fn eval<E, F>(&self, mut resolve: F) -> Result<f64, E>
    where
        F: FnMut(&VarRef) -> Result<f64, E>,
        E: From<MathEvalError>,
    {
        if self.max_stack <= INLINE_STACK {
            let mut stack = [0.0_f64; INLINE_STACK];
            self.run(&mut stack, &mut resolve)
        } else {
            let mut stack = vec![0.0_f64; self.max_stack];
            self.run(&mut stack, &mut resolve)
        }
    }

    /// Runs the program on `stack` (at least `max_stack` long).
    fn run<E, F>(&self, stack: &mut [f64], resolve: &mut F) -> Result<f64, E>
    where
        F: FnMut(&VarRef) -> Result<f64, E>,
        E: From<MathEvalError>,
    {
        let malformed = || {
            E::from(MathEvalError {
                offset: 0,
                kind: MathEvalErrorKind::MalformedProgram,
            })
        };
        let mut sp = 0usize;
        for instr in self.program.iter() {
            let (value, pop) = match instr {
                Instr::Push(v) => (*v, 0),
                Instr::Var(i) => {
                    let var = self.refs.get(*i).ok_or_else(malformed)?;
                    (resolve(var)?, 0)
                }
                Instr::Unary(op) => {
                    let a = *stack.get(sp.wrapping_sub(1)).ok_or_else(malformed)?;
                    ((op.eval)(a), 1)
                }
                Instr::Binary { op, offset } => {
                    let &[a, b] = sp
                        .checked_sub(2)
                        .and_then(|lo| stack.get(lo..sp))
                        .ok_or_else(malformed)?
                    else {
                        return Err(malformed());
                    };
                    let v = (op.eval)(a, b).map_err(|kind| {
                        E::from(MathEvalError {
                            offset: *offset,
                            kind,
                        })
                    })?;
                    (v, 2)
                }
                Instr::Call { func, argc, offset } => {
                    let args = sp
                        .checked_sub(*argc)
                        .and_then(|lo| stack.get(lo..sp))
                        .ok_or_else(malformed)?;
                    let v = (func.eval)(args).map_err(|kind| {
                        E::from(MathEvalError {
                            offset: *offset,
                            kind,
                        })
                    })?;
                    (v, *argc)
                }
            };
            sp -= pop;
            *stack.get_mut(sp).ok_or_else(malformed)? = value;
            sp += 1;
        }
        if sp == 1 {
            stack.first().copied().ok_or_else(malformed)
        } else {
            Err(malformed())
        }
    }
}
