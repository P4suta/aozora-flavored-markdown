//! Workspace automation.
//!
//! Every task invoked by the `Justfile` or by CI that isn't a direct cargo/mdbook
//! invocation lives here. Sub-commands:
//!
//! - `upstream-diff` — count lines changed against the vendored comrak tag
//!   (hard fail above 200).
//! - `upstream-sync` — fetch a new comrak tag, replace `upstream/comrak`,
//!   re-apply the afm hook patches.
//! - `corpus-refresh` — pull the pinned Aozora corpus, verify SHA256, rewrite
//!   `spec/aozora/corpus.lock`.
//! - `corpus-test` — run Tier A/B (and optionally C) against the pinned corpus.
//! - `new-adr` — scaffold a new MADR file under `docs/adr/`.
//! - `spec-refresh` — regenerate `spec/commonmark-*.json` / `spec/gfm-*.json`
//!   from cmark-format `spec.txt` inputs. Network fetching is handled by the
//!   `just spec-refresh` target (shell-side `curl`); this xtask only
//!   transforms the already-downloaded spec files into fixture JSON.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod spec_refresh;

#[derive(Parser, Debug)]
#[command(version, about = "afm workspace automation", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Count changed lines in `upstream/comrak/` against the pinned tag.
    UpstreamDiff,
    /// Sync `upstream/comrak/` to the given tag and re-apply afm hook patches.
    UpstreamSync {
        /// Upstream tag name (e.g. `v0.53.0`).
        tag: String,
    },
    /// Refresh the Aozora Bunko corpus lockfile.
    CorpusRefresh,
    /// Run the corpus regression tests at the requested tier.
    CorpusTest {
        /// Comma-separated tiers to run: `a`, `b`, `c`.
        #[arg(long, default_value = "a,b")]
        tier: String,
    },
    /// Create a new Architecture Decision Record under `docs/adr/`.
    NewAdr { title: String },
    /// Convert cmark-format spec.txt inputs to fixture JSON. Pass one or more
    /// `--from <input>=<output>` pairs. Each pair rewrites one JSON fixture.
    SpecRefresh {
        /// Input spec source file (plain text, cmark fenced-example format).
        #[arg(long)]
        input: PathBuf,
        /// Output JSON fixture path.
        #[arg(long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::UpstreamDiff => anyhow::bail!("xtask upstream-diff は未実装 (M1 E2 予定)"),
        Command::UpstreamSync { .. } => {
            anyhow::bail!("xtask upstream-sync は未実装 (M2 予定)")
        }
        Command::CorpusRefresh => anyhow::bail!("xtask corpus-refresh は未実装 (M2 予定)"),
        Command::CorpusTest { .. } => anyhow::bail!("xtask corpus-test は未実装 (M2 予定)"),
        Command::NewAdr { .. } => anyhow::bail!("xtask new-adr は未実装 (M1 E3 予定)"),
        Command::SpecRefresh { input, output } => {
            let n = spec_refresh::refresh_one(&input, &output).with_context(|| {
                format!(
                    "refreshing spec {} -> {}",
                    input.display(),
                    output.display()
                )
            })?;
            println!("spec-refresh: wrote {n} examples to {}", output.display());
            Ok(())
        }
    }
}
