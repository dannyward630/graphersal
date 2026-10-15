//! Builder methods of the vector steps (`as_vector`, `quantize`; book page "Vectors").

use crate::script::*;

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
}
