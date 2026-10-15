//! Builder methods of the vector steps (`as_vector`, `quantize`, `similarity`, `nearest`,
//! `vector_mean`; book page "Vectors").

use crate::script::tokens::ScriptVector;
use crate::script::*;
use crate::value::{Vector, VectorError, VectorMetric};

/// The script error of a vector argument in `step`.
fn vector_failure(step: &'static str, error: VectorError) -> Box<rhai::EvalAltResult> {
    script_error(crate::value::vector::vector_error(step, error))
}

/// The query vector argument: a traversal becomes a child traversal, a vector or a list of
/// numbers a constant vector; anything else fails when the step is built.
fn query_arg(
    ctx: &NativeCallContext,
    step: &'static str,
    arg: Dynamic,
) -> Result<ScriptVectorQuery, Box<rhai::EvalAltResult>> {
    if arg.is::<ScriptGraphTraversalSource>() {
        return Ok(ScriptVectorQuery::Traversal(try_dynamic_to_gts(arg)?));
    }
    if let Some(vector) = arg.read_lock::<ScriptVector>() {
        return Ok(ScriptVectorQuery::Value(vector.0.clone()));
    }
    if arg.is_array() {
        let ElementProperty::Array(items) = value_arg(ctx, arg)? else {
            return Err(vector_failure(
                step,
                VectorError::NotAVector {
                    found: "a list".to_string(),
                },
            ));
        };
        return Vector::from_values(&items)
            .map(ScriptVectorQuery::Value)
            .map_err(|error| vector_failure(step, error));
    }
    Err(vector_failure(
        step,
        VectorError::NotAVector {
            found: type_name_of(&arg),
        },
    ))
}

/// A metric name, or the script error naming the metrics.
fn metric_arg(step: &'static str, name: &str) -> Result<VectorMetric, Box<rhai::EvalAltResult>> {
    VectorMetric::from_name(name)
        .ok_or_else(|| vector_failure(step, VectorError::unknown_metric(name)))
}

/// Whether a script value names a property (a string or a `jpath(..)`).
fn is_key(arg: &Dynamic) -> bool {
    arg.is_string() || arg.is::<crate::value::JsonPath>()
}

impl ScriptGraphTraversalSource {
    /// Converts a list of numbers to an f32 vector (a vector passes). Rhai binding of
    /// `as_vector()`.
    pub fn as_vector(&mut self) -> Self {
        self.add_step(Step::AsVector)
    }

    /// Converts a vector to the element kind `kind` (`"f32"`, `"f16"`, `"i8"`, `"bit"`); an
    /// unknown kind fails when the step is built. Rhai binding of `quantize(kind)`.
    pub fn quantize(&mut self, kind: &str) -> Result<Self, Box<rhai::EvalAltResult>> {
        let element = crate::script::tokens::vector_element_of("quantize", kind)?;
        Ok(self.add_step(Step::Quantize(element)))
    }

    /// The score of the incoming vector value against `q`. Rhai binding of `similarity(q)`.
    pub fn similarity(
        ctx: NativeCallContext,
        this: &mut Self,
        q: Dynamic,
    ) -> Result<Self, Box<rhai::EvalAltResult>> {
        let query = query_arg(&ctx, "similarity", q)?;
        Ok(this.add_step(Step::Similarity(None, query, None)))
    }

    /// `similarity(key, q)` (a string or `jpath(..)` first) or `similarity(q, metric)`. Rhai
    /// binding of the two-argument overload.
    pub fn similarity_2(
        ctx: NativeCallContext,
        this: &mut Self,
        first: Dynamic,
        second: Dynamic,
    ) -> Result<Self, Box<rhai::EvalAltResult>> {
        if is_key(&first) {
            let query = query_arg(&ctx, "similarity", second)?;
            let key = dynamic_to_prop_key(first);
            return Ok(this.add_step(Step::Similarity(Some(key), query, None)));
        }
        let query = query_arg(&ctx, "similarity", first)?;
        let Some(name) = second
            .read_lock::<rhai::ImmutableString>()
            .map(|s| s.to_string())
        else {
            return Err(script_error_from_script(
                crate::error::ScriptError::ArgumentMismatch {
                    step_name: "similarity".to_string(),
                    expected: "a metric name (\"cosine\", \"dot\", \"l2\", \"hamming\")"
                        .to_string(),
                    actual: type_name_of(&second),
                },
            ));
        };
        let metric = metric_arg("similarity", &name)?;
        Ok(this.add_step(Step::Similarity(None, query, Some(metric))))
    }

    /// The score of the element's vector property `key` against `q` with `metric`. Rhai
    /// binding of `similarity(key, q, metric)`.
    pub fn similarity_3(
        ctx: NativeCallContext,
        this: &mut Self,
        key: Dynamic,
        q: Dynamic,
        metric: &str,
    ) -> Result<Self, Box<rhai::EvalAltResult>> {
        let query = query_arg(&ctx, "similarity", q)?;
        let metric = metric_arg("similarity", metric)?;
        let key = dynamic_to_prop_key(key);
        Ok(this.add_step(Step::Similarity(Some(key), query, Some(metric))))
    }

    /// The `k` incoming elements closest to `q`. Rhai binding of `nearest(key, q, k)`.
    pub fn nearest(
        ctx: NativeCallContext,
        this: &mut Self,
        key: Dynamic,
        q: Dynamic,
        k: i64,
    ) -> Result<Self, Box<rhai::EvalAltResult>> {
        Self::nearest_with(ctx, this, key, q, k, rhai::Map::new())
    }

    /// The `k` incoming elements closest to `q` with options (`#{metric: "l2"}`). Rhai binding
    /// of `nearest(key, q, k, options)`.
    pub fn nearest_with(
        ctx: NativeCallContext,
        this: &mut Self,
        key: Dynamic,
        q: Dynamic,
        k: i64,
        options: rhai::Map,
    ) -> Result<Self, Box<rhai::EvalAltResult>> {
        let k = usize::try_from(k).map_err(|_| {
            script_error_from_script(crate::error::ScriptError::ArgumentMismatch {
                step_name: "nearest".to_string(),
                expected: "a number of results k >= 0".to_string(),
                actual: k.to_string(),
            })
        })?;
        let mut metric = None;
        for (name, value) in options {
            match name.as_str() {
                "metric" => {
                    let Some(text) = value
                        .read_lock::<rhai::ImmutableString>()
                        .map(|s| s.to_string())
                    else {
                        return Err(script_error_from_script(
                            crate::error::ScriptError::ArgumentMismatch {
                                step_name: "nearest".to_string(),
                                expected:
                                    "a metric name (\"cosine\", \"dot\", \"l2\", \"hamming\")"
                                        .to_string(),
                                actual: type_name_of(&value),
                            },
                        ));
                    };
                    metric = Some(metric_arg("nearest", &text)?);
                }
                other => {
                    return Err(script_error_from_script(
                        crate::error::ScriptError::ArgumentMismatch {
                            step_name: "nearest".to_string(),
                            expected: "an option map with the key metric".to_string(),
                            actual: format!(
                                "the key {}",
                                crate::value::literal::rhai_string(other)
                            ),
                        },
                    ));
                }
            }
        }
        let query = query_arg(&ctx, "nearest", q)?;
        let key = dynamic_to_prop_key(key);
        Ok(this.add_step(Step::Nearest(key, query, k, metric)))
    }

    /// The component-wise mean of the incoming vectors. Rhai binding of `vector_mean()`.
    pub fn vector_mean(&mut self) -> Self {
        self.add_step(Step::VectorMean)
    }
}
