//! The committed feature contract must match the code that generates it.
//!
//! `models/feature_schema.json` is checked in, and `just schema` regenerates it
//! from [`admet_core::features::schema_json`]. Those two facts together create a
//! drift hazard: change the 33-feature layout in `features.rs`, forget to re-run
//! `just schema`, and the file the **Python** featuriser reads now describes a
//! layout the **Rust** featuriser no longer implements.
//!
//! That failure mode is the one ADR-01 exists to prevent, and it is the worst kind
//! available here. Nothing crashes. Both halves run happily. The model is simply
//! trained on features laid out one way and served features laid out another, and
//! the only symptom is predictions that are quietly wrong — no error, no stack
//! trace, and an accuracy regression that looks like a modelling problem.
//!
//! So the committed file is treated as a golden file, and this test is the guard.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    //  CARGO_MANIFEST_DIR is crates/admet-cli, so the root is two levels up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives two levels below the repository root")
        .to_path_buf()
}

#[test]
fn committed_feature_schema_matches_the_code() {
    let path = repo_root().join("models/feature_schema.json");

    let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\n\n\
             This file is the Python<->Rust feature contract and is committed on \
             purpose.\nRegenerate it with:  just schema",
            path.display()
        )
    });

    let generated = admet_core::features::schema_json();

    //  Compared as parsed JSON, not as text. Key order and whitespace are not part
    //  of the contract, and a test that fails on a trailing newline gets deleted by
    //  the third person it annoys.
    let committed_json: serde_json::Value =
        serde_json::from_str(&committed).expect("committed schema is valid JSON");
    let generated_json: serde_json::Value =
        serde_json::from_str(&generated).expect("generated schema is valid JSON");

    assert_eq!(
        committed_json, generated_json,
        "\n\nmodels/feature_schema.json is STALE.\n\n\
         The 33-feature layout in admet-core no longer matches the committed \
         contract that the Python featuriser reads.\n\
         If the change was intended, regenerate and bump SCHEMA_VERSION in the \
         same commit:\n\n    just schema\n\n\
         If it was not intended, this test just caught a silent train/serve skew.\n"
    );
}

/// Structural invariants of the schema itself, independent of its current values.
///
/// The equality test above would pass on two identically-broken files. These
/// assertions hold for any correct schema, so they still fail if the generator
/// starts producing something malformed.
#[test]
fn schema_blocks_tile_the_row_exactly() {
    let schema: serde_json::Value =
        serde_json::from_str(&admet_core::features::schema_json()).expect("schema is valid JSON");

    let n_features = schema["n_features"]
        .as_u64()
        .expect("n_features is a number");
    let blocks = schema["blocks"].as_array().expect("blocks is an array");

    //  Offsets must tile [0, n_features) with no gap and no overlap. A gap leaves a
    //  column permanently zero; an overlap makes two properties share a bit, and
    //  both are invisible until accuracy is inexplicably poor.
    let mut cursor = 0u64;
    for block in blocks {
        let name = block["name"].as_str().unwrap_or("<unnamed>");
        let offset = block["offset"].as_u64().expect("offset is a number");
        let width = block["width"].as_u64().expect("width is a number");

        assert_eq!(
            offset, cursor,
            "block `{name}` starts at {offset} but the previous block ended at \
             {cursor} -- gap or overlap in the feature row"
        );
        assert!(width > 0, "block `{name}` has zero width");
        cursor += width;
    }

    assert_eq!(
        cursor, n_features,
        "blocks cover {cursor} columns but n_features is {n_features}"
    );

    //  The number the ONNX graph is built around. If this ever changes, the
    //  exported model and every fixture change with it.
    assert_eq!(n_features, 33, "the feature contract is 33-dimensional");
}

/// One-hot orderings are part of the contract, not incidental.
///
/// The Python side will index these lists positionally. Reordering `element_order`
/// without bumping `SCHEMA_VERSION` would silently relabel every atom — carbon
/// becoming nitrogen as far as the model is concerned.
#[test]
fn one_hot_orderings_match_their_block_widths() {
    let schema: serde_json::Value =
        serde_json::from_str(&admet_core::features::schema_json()).expect("schema is valid JSON");

    let width_of = |name: &str| -> u64 {
        schema["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["name"] == name)
            .unwrap_or_else(|| panic!("no block named `{name}`"))["width"]
            .as_u64()
            .unwrap()
    };

    for (block, list) in [
        ("element", "element_order"),
        ("hybridisation", "hybridisation_order"),
        ("formal_charge", "charge_buckets"),
        ("degree", "degree_buckets"),
        ("num_hs", "hydrogen_buckets"),
    ] {
        let len = schema[list]
            .as_array()
            .unwrap_or_else(|| panic!("`{list}` is not an array"))
            .len() as u64;
        assert_eq!(
            len,
            width_of(block),
            "`{list}` has {len} entries but block `{block}` is {} wide -- the \
             Python side indexes this list positionally",
            width_of(block)
        );
    }

    //  DEF-04 specifically: `Hybridisation::Unknown` is encoded as sp3, so index 2
    //  of this list must BE sp3. When the generic `min(width - 1)` clamp was in
    //  charge it landed on sp3d2 instead, labelling every atom of undetermined
    //  hybridisation as octahedral.
    assert_eq!(
        schema["hybridisation_order"][2], "sp3",
        "Unknown hybridisation clamps to index 2, which must be sp3 (DEF-04)"
    );
}
