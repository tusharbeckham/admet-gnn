//! ADMETriage command-line interface.
//!
//! Manual chapter 22. Three subcommands, each earning its place:
//!
//! ```text
//! admet-cli predict "CC(=O)Oc1ccccc1C(=O)O"        one molecule, human output
//! admet-cli import  compounds.csv -o scored.csv    batch, no server needed
//! admet-cli bench   --n 1000                       throughput, no HTTP in the way
//! ```
//!
//! # Why a CLI exists at all when there is an API
//!
//! Three concrete reasons, in order of how often they matter.
//!
//! 1. **Bisecting failures.** When a prediction looks wrong, the question is
//!    whether the model is wrong or the service is. The CLI runs the identical
//!    chemistry and the identical `.onnx` with no HTTP, no cache and no database,
//!    so one command answers it.
//! 2. **Honest benchmarks.** `bench` measures parse + featurise + infer. An HTTP
//!    benchmark measures those plus serialisation, middleware, and the loopback
//!    interface, and reporting that as model latency overstates it.
//! 3. **Demonstrations that cannot fail.** A live demo depending on a running
//!    Postgres, a running server and a browser has three ways to embarrass you.
//!    `admet-cli predict aspirin` has none.
//!
//! # Scaffold status
//!
//! Argument parsing is complete and `--help` is accurate. The command bodies
//! depend on the SMILES parser (Increment 2) and a trained model (Increment 1),
//! so each prints what it *will* do and exits non-zero. Non-zero matters: a stub
//! that exits 0 makes a shell script think it worked.

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

/// Top-level arguments.
#[derive(Debug, Parser)]
#[command(
    name = "admet-cli",
    version,
    about = "Offline ADMET screening: parse, featurise, score, rank",
    long_about = None,
)]
struct Cli {
    /// Path to the ONNX model artefact.
    #[arg(long, default_value = "models/model.onnx", global = true)]
    model: PathBuf,

    /// ONNX Runtime intra-op threads. One is usually right for a single request;
    /// batch work gets its parallelism from rayon across rows instead, and
    /// stacking both oversubscribes the CPU and makes throughput worse.
    #[arg(long, default_value_t = 1, global = true)]
    threads: usize,

    /// Increase log verbosity. Repeatable: `-v` for debug, `-vv` for trace.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Score a single molecule and print a human-readable report.
    Predict {
        /// SMILES string.
        smiles: String,
        /// Emit JSON instead of a table. The same payload the API returns, so a
        /// script can consume either interchangeably.
        #[arg(long)]
        json: bool,
    },

    /// Score a CSV of molecules.
    Import {
        /// Input CSV.
        input: PathBuf,
        /// Output CSV. Defaults to stdout, which makes the tool composable.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Column holding the SMILES.
        #[arg(long, default_value = "smiles")]
        smiles_column: String,
        /// Keep going past unparseable rows, recording the reason per row.
        ///
        /// Default **on**. A 10,000-row screen that aborts at row 4,000 because
        /// one vendor catalogue entry has a stray character has wasted forty
        /// minutes; the rows that failed are themselves a finding worth
        /// reporting, not a reason to stop.
        #[arg(long, default_value_t = true)]
        skip_invalid: bool,
        /// Rows per progress line. 0 silences progress.
        #[arg(long, default_value_t = 250)]
        progress_every: usize,
    },

    /// Benchmark the pipeline.
    Bench {
        /// Molecules to score.
        #[arg(long, default_value_t = 1_000)]
        n: usize,
        /// Micro-batch size. Sweep this to reproduce the figure in the
        /// performance chapter rather than asserting a knee that was never
        /// measured.
        #[arg(long, default_value_t = 64)]
        batch: usize,
        /// Report per-stage timings (parse, featurise, infer) rather than only
        /// the total. Which stage dominates is the question worth answering, and
        /// a single end-to-end number cannot.
        #[arg(long)]
        breakdown: bool,
    },

    /// Print the 33-feature schema as JSON.
    ///
    /// Not a placeholder -- this one works today, because it needs no model and no
    /// parser. It is how the Python featuriser and the Rust featuriser stay in
    /// step: one source of truth, exported rather than duplicated. See TR-03.
    Schema,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    match &cli.command {
        Command::Schema => {
            // The only command with a real body in the scaffold, and the one that
            // makes the parity contract enforceable from day one.
            println!("{}", admet_core::features::schema_json());
            Ok(())
        }

        Command::Predict { smiles, json } => {
            eprintln!("predict: {} ({} chars)", smiles, smiles.len());
            eprintln!("  json output: {json}");
            eprintln!("  model: {}", cli.model.display());
            // Validation works now, so a malformed input is caught before the
            // "not implemented" message -- which means this command already tells
            // you something true about your input.
            match admet_core::validate_input(smiles) {
                Ok(()) => eprintln!("  input passes cheap validation"),
                Err(e) => bail!("input rejected before parsing: {e}"),
            }
            bail!("predict needs the SMILES parser (Increment 2) and a trained model (Increment 1)")
        }

        Command::Import {
            input,
            output,
            smiles_column,
            skip_invalid,
            progress_every,
        } => {
            if !input.exists() {
                bail!("input file does not exist: {}", input.display());
            }
            eprintln!("import: {}", input.display());
            eprintln!("  smiles column:  {smiles_column}");
            eprintln!(
                "  output:         {}",
                output
                    .as_ref()
                    .map_or("<stdout>".into(), |p| p.display().to_string())
            );
            eprintln!("  skip invalid:   {skip_invalid}");
            eprintln!("  progress every: {progress_every}");
            bail!("import needs the SMILES parser (Increment 2) and a trained model (Increment 1)")
        }

        Command::Bench {
            n,
            batch,
            breakdown,
        } => {
            eprintln!("bench: {n} molecules, batch {batch}, breakdown {breakdown}");
            eprintln!("  threads: {}", cli.threads);
            eprintln!(
                "  note: `cargo bench -p admet-core` already benchmarks the pure functions \
                 (Tanimoto, top-k, domain search). This command benchmarks the whole pipeline \
                 including inference, and needs a model to do it."
            );
            bail!("bench needs a trained model (Increment 1)")
        }
    }
}

/// Map `-v` count to a filter, unless `RUST_LOG` already says otherwise.
///
/// `RUST_LOG` wins on purpose: someone who has set it has been more specific
/// than a flag can be, and silently overriding that is the kind of small
/// disobedience that wastes half an hour.
fn init_logging(verbosity: u8) {
    let default = match verbosity {
        0 => "warn",
        1 => "info,admet_cli=debug",
        _ => "debug,admet_cli=trace",
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default));
    // Logs go to stderr so stdout stays a clean data channel: `admet-cli import
    // x.csv | head` must not have log lines interleaved into the CSV.
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// clap's own consistency check: conflicting flags, duplicate short options,
    /// invalid defaults. Cheap to run and it catches the class of mistake that
    /// otherwise surfaces as a panic the first time a user passes `--help`.
    #[test]
    fn the_argument_parser_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    /// Global flags must be accepted after the subcommand as well as before it.
    /// Users type them in both positions and being strict about it is a papercut
    /// with no upside.
    #[test]
    fn global_flags_work_on_either_side_of_the_subcommand() {
        let a = Cli::try_parse_from(["admet-cli", "--threads", "4", "schema"]).unwrap();
        let b = Cli::try_parse_from(["admet-cli", "schema", "--threads", "4"]).unwrap();
        assert_eq!(a.threads, 4);
        assert_eq!(b.threads, 4);
    }

    /// Defaults are part of the interface. If `skip_invalid` ever defaults to
    /// false, a 10,000-row import starts aborting on the first bad row.
    #[test]
    fn import_defaults_are_the_forgiving_ones() {
        let cli = Cli::try_parse_from(["admet-cli", "import", "in.csv"]).unwrap();
        match cli.command {
            Command::Import {
                skip_invalid,
                progress_every,
                smiles_column,
                output,
                ..
            } => {
                assert!(skip_invalid, "a batch must survive one bad row");
                assert_eq!(progress_every, 250);
                assert_eq!(smiles_column, "smiles");
                assert!(
                    output.is_none(),
                    "stdout by default keeps the tool composable"
                );
            }
            other => panic!("expected Import, got {other:?}"),
        }
    }

    /// `schema` must work with no model present -- it is the parity contract, and
    /// requiring a trained model to read a schema would be circular.
    #[test]
    fn the_schema_is_valid_json_and_names_its_version() {
        let json = admet_core::features::schema_json();
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("schema must be valid JSON");
        assert_eq!(
            parsed["schema_version"],
            admet_core::features::SCHEMA_VERSION
        );
        assert_eq!(parsed["n_features"], admet_core::N_ATOM_FEATURES as u64);
        assert_eq!(
            parsed["max_heavy_atoms"],
            admet_core::MAX_HEAVY_ATOMS as u64
        );
        let blocks = parsed["blocks"]
            .as_array()
            .expect("blocks must be an array");
        let total: u64 = blocks.iter().map(|b| b["width"].as_u64().unwrap()).sum();
        assert_eq!(
            total,
            admet_core::N_ATOM_FEATURES as u64,
            "the exported block widths must sum to the feature count, or the Python \
             featuriser will build a row of the wrong length (TR-03)"
        );
    }
}
