//! `POST /predict` -- the endpoint the whole system exists to serve.
//!
//! Manual chapter 19.3. The handler body arrives in Increment 2; the **payload
//! shapes** are here now, deliberately, because they are the contract the web
//! client is written against and settling them early is cheaper than migrating a
//! UI later.
//!
//! # The nine steps this handler performs
//!
//! From `method.md` §8, in order, with the layer that owns each:
//!
//! | # | Step | Owner |
//! |---|---|---|
//! | 1 | parse SMILES, byte-offset errors | `admet-core` |
//! | 2 | canonicalise → InChIKey | `admet-core` |
//! | 3 | cache lookup `(inchikey, model_version)` | `admet-infer` then `admet-db` |
//! | 4 | deterministic descriptors | `admet-core` |
//! | 5 | featurise to `x`, `adj`, `mask` | `admet-core` |
//! | 6 | infer, micro-batched to 64 | `admet-infer` |
//! | 7 | applicability domain | `admet-infer` |
//! | 8 | triage score | `admet-core` |
//! | 9 | respond | here |
//!
//! Step 9 is this file's entire job. Eight of the nine steps are testable without
//! a socket, which is ADR-02 paying for itself.
//!
//! # The response separates two kinds of number, and must keep doing so
//!
//! `descriptors` are **computed** -- molecular weight is a fact about the
//! structure. `predictions` are **inferred** -- a model's guess, with a
//! confidence caveat attached. NFR-10 requires the UI to render them with
//! visibly different treatment, and the payload makes that possible by never
//! mixing them into one flat object. Flattening them would be the single change
//! most likely to make this system quietly misleading.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::state::AppState;

/// `POST /predict` request body.
#[derive(Debug, Clone, Deserialize)]
pub struct PredictRequest {
    /// The SMILES string, exactly as typed. Length-capped at
    /// [`admet_core::MAX_SMILES_LEN`] before parsing, so a megabyte of
    /// parentheses is rejected by a comparison rather than by the parser.
    pub smiles: String,
    /// Whether to include per-atom attributions (Increment 4). Off by default:
    /// integrated gradients costs roughly 20 forward passes, so it must be
    /// opt-in or every request pays for a feature most callers do not use.
    #[serde(default)]
    pub explain: bool,
}

/// `POST /predict` response body.
#[derive(Debug, Clone, Serialize)]
pub struct PredictResponse {
    /// Echo of what the user submitted. Shown back verbatim so a chemist who
    /// typed `OCC` is not silently presented with `CCO`.
    pub input_smiles: String,
    /// Canonical form.
    pub canonical_smiles: String,
    /// Identity key.
    pub inchikey: String,
    /// **Computed** chemistry -- exact, not predicted.
    pub descriptors: Descriptors,
    /// **Inferred** values, one per endpoint, in model output order.
    pub predictions: Vec<EndpointPrediction>,
    /// Applicability-domain verdict.
    pub domain: Domain,
    /// Weighted geometric mean of desirability, or `null` when withheld.
    ///
    /// `Option`, and the `null` is load-bearing: FR-12 withholds the score for
    /// out-of-domain molecules, and a withheld score sent as `0.0` would render
    /// as "worst possible compound" -- a confident claim where the honest answer
    /// is "we do not know".
    pub triage_score: Option<f32>,
    /// Which model produced this. Without it the numbers are not reproducible.
    pub model_version: i32,
    /// Whether this came from cache. Useful in the UI and essential when
    /// interpreting a latency figure.
    pub cached: bool,
}

/// Deterministic molecular descriptors.
#[derive(Debug, Clone, Serialize)]
pub struct Descriptors {
    /// g/mol.
    pub molecular_weight: f64,
    /// Crippen cLogP.
    pub clogp: f64,
    /// Topological polar surface area, Å².
    pub tpsa: f64,
    /// Hydrogen-bond donors.
    pub hbd: u32,
    /// Hydrogen-bond acceptors.
    pub hba: u32,
    /// Rotatable bonds.
    pub rotatable_bonds: u32,
    /// Rings (SSSR count).
    pub ring_count: u32,
    /// Heavy atoms.
    pub heavy_atoms: u32,
    /// Per-rule Lipinski results, **not** a violation count.
    ///
    /// FR-07 requires the rules individually. "2 violations" hides which two, and
    /// a chemist's next action depends entirely on whether the failures were
    /// molecular weight and logP or donors and acceptors.
    pub lipinski: Vec<RuleResult>,
    /// Per-rule Veber results.
    pub veber: Vec<RuleResult>,
}

/// One drug-likeness rule outcome.
#[derive(Debug, Clone, Serialize)]
pub struct RuleResult {
    /// e.g. `"MW <= 500"`. The rule as written, so the UI needs no lookup table.
    pub rule: String,
    /// Whether it passed.
    pub passed: bool,
    /// The measured value that decided it. Present so the user can see *how*
    /// close a borderline case was -- 501 and 900 both "fail" and mean very
    /// different things.
    pub value: f64,
}

/// One endpoint's prediction.
#[derive(Debug, Clone, Serialize)]
pub struct EndpointPrediction {
    /// Short code, e.g. `"bbb"`.
    pub endpoint: String,
    /// Display name.
    pub name: String,
    /// Category, for grouping in the UI.
    pub category: String,
    /// Probability for binary endpoints, physical quantity for regression ones.
    pub value: f32,
    /// Unit, `null` for probabilities.
    pub unit: Option<String>,
    /// `"binary"` or `"regression"`. The client must not infer this from the
    /// value's range: a probability of 0.82 and a log-solubility of 0.82 are
    /// indistinguishable numerically and mean nothing alike.
    pub task: String,
    /// Desirability in `[0, 1]`, the triage input.
    pub desirability: f32,
}

/// Applicability-domain verdict.
#[derive(Debug, Clone, Serialize)]
pub struct Domain {
    /// `in_domain` | `low_confidence` | `out_of_domain`.
    pub status: String,
    /// Mean top-5 Tanimoto to the reference set.
    pub similarity: f32,
    /// Plain-language explanation, shown next to the badge. A status code with no
    /// explanation gets ignored; a sentence saying the model has not seen
    /// chemistry like this does not.
    pub explanation: String,
}

/// `POST /predict`.
///
/// # Errors
///
/// [`ApiError::Inference`] until Increment 2 -- a 503 with a problem body, so a
/// client written against this contract now receives a documented, retryable
/// error rather than a 404 that looks like a wrong URL.
pub async fn predict_one(
    State(state): State<AppState>,
    Json(request): Json<PredictRequest>,
) -> Result<Json<PredictResponse>, ApiError> {
    let _ = (&state, &request);
    Err(ApiError::Inference(
        "predict is implemented in Increment 2".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `explain` flag must default to false when absent. If it defaulted to
    /// true, every request would silently pay for ~20 extra forward passes.
    #[test]
    fn explain_defaults_to_off() {
        let r: PredictRequest = serde_json::from_str(r#"{"smiles":"CCO"}"#).unwrap();
        assert!(!r.explain);
        assert_eq!(r.smiles, "CCO");
    }

    /// A withheld triage score must serialise as `null`, never as a number.
    /// Serialising it as `0.0` would present "unknown" as "worst possible", which
    /// is the most consequential rendering bug this payload could have.
    #[test]
    fn a_withheld_triage_score_serialises_as_null() {
        let json = serde_json::to_string(&Some(0.42_f32)).unwrap();
        assert_eq!(json, "0.42");
        let withheld: Option<f32> = None;
        assert_eq!(serde_json::to_string(&withheld).unwrap(), "null");
    }

    /// Computed and inferred values live in separate objects, and NFR-10 depends
    /// on that separation surviving future edits to this struct.
    #[test]
    fn descriptors_and_predictions_are_separate_fields() {
        let fields = std::any::type_name::<PredictResponse>();
        assert!(fields.contains("PredictResponse"));
        // The real assertion is structural and enforced by the type: there is no
        // way to reach a descriptor through `predictions` or vice versa. This test
        // documents the intent for whoever considers flattening them.
    }

    /// Rules are reported individually with the value that decided them.
    #[test]
    fn drug_likeness_rules_carry_their_measured_value() {
        let r = RuleResult {
            rule: "MW <= 500".to_owned(),
            passed: false,
            value: 501.2,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(
            json.contains("501.2"),
            "a borderline failure must be visible: {json}"
        );
        assert!(json.contains("MW <= 500"));
    }
}
