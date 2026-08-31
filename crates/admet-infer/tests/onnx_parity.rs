//! Python <-> Rust inference parity.
//!
//! This is the test that closes the loop. `spike_onnx_export.py` proved
//! PyTorch agrees with ONNX Runtime *in Python*. This proves ONNX Runtime
//! *in Rust* agrees with ONNX Runtime in Python, on the same committed
//! artefact, at more than one batch size.
//!
//! Together they mean: whatever the trainer produces, the server serves.
//! Without this test, a discrepancy between the two halves shows up as a
//! mysteriously worse AUROC in production, weeks later, with no obvious
//! cause -- because both halves individually "work".
//!
//! Regenerate the fixture with:
//!     python training/scripts/dump_parity_fixture.py
//!
//! Never regenerate it to make this test pass. If the numbers moved, either
//! the model changed (fine, regenerate deliberately) or something broke
//! (not fine, and hiding it costs you the rest of the project).

use std::path::{Path, PathBuf};

use admet_infer::Engine;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Manifest {
    model: String,
    tolerance: f32,
    dtype: String,
    output_name: String,
    input_names: Vec<String>,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    batch: usize,
    shapes: Vec<Vec<usize>>,
    inputs: Vec<String>,
    expected: String,
    expected_shape: Vec<usize>,
}

/// Repository root, derived from the crate location rather than the working
/// directory. `cargo test` runs with CWD set to the crate root, but that is
/// an implementation detail and it differs under some runners.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root should resolve")
}

/// Read a raw little-endian f32 blob.
fn read_f32(path: &Path) -> Vec<f32> {
    let bytes =
        std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    assert_eq!(
        bytes.len() % 4,
        0,
        "{} is {} bytes, not a whole number of f32",
        path.display(),
        bytes.len()
    );
    //  `as_chunks::<4>` over `chunks_exact(4)`: the chunk width becomes a const
    //  generic, so each chunk is a `&[u8; 4]` and `from_le_bytes` takes it
    //  directly instead of being hand-indexed. One fewer place to typo an index.
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

#[test]
fn rust_matches_python_on_the_committed_model() {
    let root = repo_root();
    let dir = root.join("fixtures").join("parity");
    let manifest_path = dir.join("manifest.json");

    if !manifest_path.exists() {
        panic!(
            "missing {}\n\nGenerate it first:\n    \
             python training/scripts/dump_parity_fixture.py",
            manifest_path.display()
        );
    }

    let manifest: Manifest =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("manifest readable"))
            .expect("manifest parses");

    assert_eq!(
        manifest.dtype, "float32-le",
        "fixture dtype changed; this test only understands float32-le"
    );

    let model = root.join(&manifest.model);
    let mut engine =
        Engine::load(&model, 1).unwrap_or_else(|e| panic!("cannot load {}: {e}", model.display()));

    // Fail on a renamed or reordered graph rather than on numbers. A shape
    // mismatch is a confusing way to learn that the exporter renamed `mask`.
    assert_eq!(
        engine.input_names(),
        manifest.input_names,
        "graph inputs differ from the fixture -- regenerate the fixture"
    );
    assert!(
        engine.output_names().contains(&manifest.output_name),
        "graph has no output `{}`; it has {:?}",
        manifest.output_name,
        engine.output_names()
    );

    println!("\nmodel     {}", manifest.model);
    println!("tolerance {:.1e}\n", manifest.tolerance);

    let mut worst = 0.0f32;

    for case in &manifest.cases {
        assert_eq!(
            case.inputs.len(),
            case.shapes.len(),
            "case {} lists {} inputs but {} shapes",
            case.batch,
            case.inputs.len(),
            case.shapes.len()
        );

        let inputs: Vec<(Vec<usize>, Vec<f32>)> = case
            .inputs
            .iter()
            .zip(&case.shapes)
            .map(|(file, shape)| (shape.clone(), read_f32(&dir.join(file))))
            .collect();

        let got = engine
            .run(&inputs, &manifest.output_name)
            .unwrap_or_else(|e| panic!("inference failed at batch {}: {e}", case.batch));

        let expect = read_f32(&dir.join(&case.expected));
        let want: usize = case.expected_shape.iter().product();

        assert_eq!(
            expect.len(),
            want,
            "expected blob for batch {} has {} values, shape {:?} needs {}",
            case.batch,
            expect.len(),
            case.expected_shape,
            want
        );
        assert_eq!(
            got.len(),
            expect.len(),
            "batch {}: rust produced {} values, python produced {}",
            case.batch,
            got.len(),
            expect.len()
        );

        // Absolute difference, not relative. These are raw logits centred
        // near zero, so a relative measure would blow up harmlessly on the
        // elements that happen to sit closest to 0.
        let mut max_diff = 0.0f32;
        let mut at = 0usize;
        for (i, (g, e)) in got.iter().zip(&expect).enumerate() {
            let d = (g - e).abs();
            if d > max_diff {
                max_diff = d;
                at = i;
            }
        }
        worst = worst.max(max_diff);

        println!(
            "batch {:<3} {:>5} values   max abs diff {:.3e}   {}",
            case.batch,
            got.len(),
            max_diff,
            if max_diff <= manifest.tolerance {
                "ok"
            } else {
                "FAIL"
            }
        );

        assert!(
            max_diff <= manifest.tolerance,
            "\nPARITY FAILED at batch {}\n\
             element {}: rust {:.9}  python {:.9}  diff {:.3e}  tol {:.1e}\n\n\
             Diagnose in this order:\n\
             1. Are you loading the same file? Check fixtures/spike_tiny_gin.onnx\n\
                against the `model` field in the fixture manifest.\n\
             2. Did the fixture go stale? Re-running dump_parity_fixture.py is\n\
                correct ONLY if the model genuinely changed.\n\
             3. Is an input transposed? A wrong-but-plausible layout gives\n\
                differences around 1e-1, not 1e-7. Tiny diffs mean numerics;\n\
                large diffs mean wiring.\n\
             4. Is `adj` symmetric and normalised the way Python wrote it?\n\
                Rust reads bytes, so this can only break if the shapes lie.\n",
            case.batch,
            at,
            got[at],
            expect[at],
            max_diff,
            manifest.tolerance
        );
    }

    println!(
        "\nPARITY PASSED -- worst {:.3e} across {} batch sizes\n",
        worst,
        manifest.cases.len()
    );
}
