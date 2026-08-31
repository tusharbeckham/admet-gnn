//! ONNX Runtime inference for ADMETriage.
//!
//! This crate owns exactly one responsibility: turn a batch of already-
//! featurised molecules into endpoint logits, by way of an ONNX graph. It
//! knows nothing about SMILES, chemistry, HTTP, or databases. That separation
//! is the whole reason the ONNX boundary exists.
//!
//! # The contract
//!
//! The graph exported by `training/scripts/spike_onnx_export.py` takes three
//! inputs, in this order, and produces one output:
//!
//! | name          | shape             | meaning                                |
//! |---------------|-------------------|----------------------------------------|
//! | `x`           | `[B, 128, 33]`    | atom feature matrix, zero-padded       |
//! | `adj`         | `[B, 128, 128]`   | `D^-1/2 (A + I) D^-1/2`, zero-padded   |
//! | `mask`        | `[B, 128]`        | 1.0 for real atoms, 0.0 for padding    |
//! | `predictions` | `[B, 12]`         | raw logits, one per endpoint           |
//!
//! Only the batch axis is dynamic. The atom axis is fixed at 128 because that
//! is what makes the graph exportable at all -- see ADR-01 in research.md.
//! Molecules larger than 128 heavy atoms are rejected upstream, never
//! truncated: a truncated molecule is a different molecule, and silently
//! predicting properties for a different molecule is worse than refusing.

use std::path::Path;

use ort::session::{builder::GraphOptimizationLevel, builder::SessionBuilder, Session};
use ort::value::{DynValue, Tensor};

/// Maximum heavy atoms per molecule. Fixed in the exported graph.
pub const MAX_ATOMS: usize = 128;
/// Atom feature vector width. See the 33-feature table in method.md 3a.
pub const N_FEATURES: usize = 33;
/// Number of ADMET endpoints predicted per molecule.
pub const N_ENDPOINTS: usize = 12;

/// Everything that can go wrong between a feature matrix and a logit.
#[derive(Debug, thiserror::Error)]
pub enum InferError {
    #[error("onnx runtime: {0}")]
    Ort(#[from] ort::Error),

    //  `ort` 2.0.0-rc.13 makes its error generic over the operation that
    //  failed (`Error<R>`), so the builder chain yields
    //  `Error<SessionBuilder>` rather than `Error<()>`. The crate provides
    //  `From<Error<SessionBuilder>> for Error<()>`, but `?` will not chain two
    //  conversions, so the builder error needs its own variant. Naming it
    //  separately is also more useful than flattening: "failed to build the
    //  session" and "failed to run the graph" have different causes.
    //  BOXED, and not incidentally: `Error<SessionBuilder>` hands the builder
    //  itself back to the caller so a failed configuration step can be
    //  retried, which makes it 144 bytes. Inlining that into the enum would
    //  pay for it on the size of EVERY `Result` this crate returns, including
    //  the hot `run` path. `clippy::result_large_err` is right to object.
    #[error("onnx runtime (session builder): {0}")]
    OrtBuilder(Box<ort::Error<SessionBuilder>>),

    #[error("input {index} has {len} values but shape {shape:?} needs {want}")]
    LengthMismatch {
        index: usize,
        len: usize,
        shape: Vec<usize>,
        want: usize,
    },

    #[error("model has no output named `{0}`")]
    MissingOutput(String),

    #[error("model wants {wanted} inputs {names:?} but {given} were supplied")]
    ArityMismatch {
        given: usize,
        wanted: usize,
        names: Vec<String>,
    },
}

pub type Result<T> = std::result::Result<T, InferError>;

//  Hand-written because `#[from]` would generate `From<Box<Error<..>>>`, and
//  `?` at the call site has an unboxed error. This is the one line that keeps
//  the builder chain readable.
impl From<ort::Error<SessionBuilder>> for InferError {
    fn from(err: ort::Error<SessionBuilder>) -> Self {
        Self::OrtBuilder(Box::new(err))
    }
}

/// A loaded ONNX model, ready to score batches.
///
/// Construction is expensive -- it initialises the ONNX Runtime environment,
/// reads the graph, and runs optimisation passes. Do it once at startup and
/// share it. Do NOT build one per request; that is the single easiest way to
/// turn a 4 ms prediction into a 400 ms one.
pub struct Engine {
    session: Session,
}

impl Engine {
    /// Load a model from disk.
    ///
    /// `intra_threads` bounds the thread pool used inside a single inference.
    /// On a 2 vCPU serving box, 1 is usually the right answer: the concurrency
    /// that matters comes from serving many requests at once, not from
    /// splitting one small graph across cores. Oversubscribing here makes p99
    /// worse, not better.
    pub fn load(path: impl AsRef<Path>, intra_threads: usize) -> Result<Self> {
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(intra_threads)?
            .commit_from_file(path.as_ref())?;
        Ok(Self { session })
    }

    /// Input names in graph order, for diagnostics.
    pub fn input_names(&self) -> Vec<String> {
        self.session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect()
    }

    /// Output names in graph order, for diagnostics.
    pub fn output_names(&self) -> Vec<String> {
        self.session
            .outputs()
            .iter()
            .map(|o| o.name().to_string())
            .collect()
    }

    /// Run the graph.
    ///
    /// `inputs` are `(shape, values)` pairs in **graph order**. Positional
    /// rather than named on purpose: the exporter is free to rename graph
    /// inputs between torch versions, and a positional contract fails loudly
    /// at the shape check instead of quietly feeding `adj` into `x`.
    ///
    /// Returns the named output flattened row-major.
    pub fn run(&mut self, inputs: &[(Vec<usize>, Vec<f32>)], output: &str) -> Result<Vec<f32>> {
        //  `ort` rc.13 does not export `SessionInputValue`, so a purely
        //  positional feed (`&[SessionInputValue]`) cannot be built from
        //  outside the crate. The positional CONTRACT is preserved anyway by
        //  reading the graph's own input names in order and binding argument
        //  `i` to name `i`: the caller still never types a name, and a graph
        //  whose inputs were renamed upstream still gets fed correctly.
        let names: Vec<String> = self
            .session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();

        if names.len() != inputs.len() {
            return Err(InferError::ArityMismatch {
                given: inputs.len(),
                wanted: names.len(),
                names,
            });
        }

        let mut feeds: Vec<(String, DynValue)> = Vec::with_capacity(inputs.len());

        for (index, (shape, data)) in inputs.iter().enumerate() {
            let want: usize = shape.iter().product();
            if want != data.len() {
                return Err(InferError::LengthMismatch {
                    index,
                    len: data.len(),
                    shape: shape.clone(),
                    want,
                });
            }
            //  `(shape, Vec<T>)` is a first-class tensor source in `ort`, so the
            //  ndarray round-trip this used to do was pure overhead -- and worse,
            //  it coupled this crate to whichever ndarray version `ort` happened
            //  to be built against. Dropping it removes an entire class of
            //  version-mismatch breakage (risk R6).
            let tensor = Tensor::from_array((shape.clone(), data.clone()))?;
            feeds.push((names[index].clone(), tensor.into_dyn()));
        }

        let outputs = self.session.run(feeds)?;
        let (_shape, slice) = outputs
            .get(output)
            .ok_or_else(|| InferError::MissingOutput(output.to_string()))?
            .try_extract_tensor::<f32>()?;

        Ok(slice.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_constants_match_the_exported_graph() {
        // Tripwire, not a tautology. If someone edits these to "support
        // bigger molecules" without re-exporting the model, the numbers here
        // and the numbers baked into the ONNX file diverge, and every
        // prediction silently becomes garbage. The real defence is the parity
        // test in tests/onnx_parity.rs; this just documents the intent.
        assert_eq!(MAX_ATOMS, 128);
        assert_eq!(N_FEATURES, 33);
        assert_eq!(N_ENDPOINTS, 12);
    }
}
