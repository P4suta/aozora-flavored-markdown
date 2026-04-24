//! Workspace automation.
//!
//! Every task invoked by the `Justfile` or by CI that isn't a direct cargo/mdbook
//! invocation lives here. Sub-commands:
//!
//! - `upstream-diff` — assert the vendored comrak tree is still pinned
//!   to the recorded SHA and that the 200-line diff policy from ADR-0001
//!   is documented in `upstream/comrak/UPSTREAM_DIFF.md`.
//! - `upstream-sync` — *deferred*: fetch a new comrak tag, replace
//!   `upstream/comrak`, re-apply the afm hook patches.
//! - `corpus-refresh` — *deferred*: pull the pinned Aozora corpus,
//!   verify SHA256, rewrite `spec/aozora/corpus.lock`.
//! - `corpus-test` — *deferred*: run Tier A/B (and optionally C)
//!   against the pinned corpus.
//! - `new-adr` — scaffold a new MADR file under `docs/adr/`.
//! - `spec-refresh` — regenerate `spec/commonmark-*.json` / `spec/gfm-*.json`
//!   from cmark-format `spec.txt` inputs. Network fetching is handled by the
//!   `just spec-refresh` target (shell-side `curl`); this xtask only
//!   transforms the already-downloaded spec files into fixture JSON.
//!
//! Deferred sub-commands return a clear error message rather than a
//! generic bail so the `Justfile` wrappers surface actionable intent
//! instead of a mysterious failure.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};

mod spec_refresh;

/// ADR-0001 upstream diff budget, in lines. Changing this requires an ADR.
const UPSTREAM_DIFF_BUDGET_LINES: usize = 200;

#[derive(Parser, Debug)]
#[command(version, about = "afm workspace automation", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Verify the ADR-0001 upstream-diff policy is in force.
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
        Command::UpstreamDiff => upstream_diff(),
        Command::UpstreamSync { tag } => {
            Err(deferred("upstream-sync", &format!("requested tag: {tag}")))
        }
        Command::CorpusRefresh => Err(deferred("corpus-refresh", "")),
        Command::CorpusTest { tier } => {
            Err(deferred("corpus-test", &format!("requested tiers: {tier}")))
        }
        Command::NewAdr { title } => new_adr(&title),
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

/// Verify the ADR-0001 upstream-diff policy is in force.
///
/// Byte-level enforcement (fetch upstream at the pinned SHA, diff against
/// the vendored tree, hard-fail on overage) is scheduled for a follow-up
/// ADR. For the current CI gate we assert the policy documentation exists
/// and is internally consistent — the contract a human reviewer would
/// enforce on any PR touching `upstream/comrak/`.
fn upstream_diff() -> Result<()> {
    let sha_path = PathBuf::from("upstream/comrak/COMRAK_SHA");
    let raw =
        fs::read_to_string(&sha_path).with_context(|| format!("reading {}", sha_path.display()))?;

    // COMRAK_SHA is two lines: the pinned commit SHA on line 1, the
    // upstream tag name (e.g. "v0.52.0") on line 2. Both are optional to
    // mention in the output but the SHA is required for the gate.
    let mut lines = raw.lines().map(str::trim).filter(|s| !s.is_empty());
    let sha = lines.next().unwrap_or_default();
    let tag = lines.next().unwrap_or_default();
    if sha.is_empty() {
        bail!("{} is empty", sha_path.display());
    }

    let diff_md_path = PathBuf::from("upstream/comrak/UPSTREAM_DIFF.md");
    let diff_md = fs::read_to_string(&diff_md_path)
        .with_context(|| format!("reading {}", diff_md_path.display()))?;

    if !diff_md.contains(&UPSTREAM_DIFF_BUDGET_LINES.to_string()) {
        bail!(
            "{} does not mention the {}-line upstream diff budget (ADR-0001)",
            diff_md_path.display(),
            UPSTREAM_DIFF_BUDGET_LINES,
        );
    }

    if tag.is_empty() {
        println!("upstream-diff: vendored comrak pinned at {sha}");
    } else {
        println!("upstream-diff: vendored comrak pinned at {sha} ({tag})");
    }
    println!(
        "upstream-diff: budget {UPSTREAM_DIFF_BUDGET_LINES} lines (ADR-0001), policy documented in {}",
        diff_md_path.display()
    );
    println!(
        "upstream-diff: NOTE — byte-level enforcement via network fetch + diff \
         is scheduled for a follow-up ADR. This gate currently verifies the \
         policy documentation is in force."
    );

    Ok(())
}

/// Scaffold a new Architecture Decision Record under `docs/adr/`.
///
/// Picks the next available four-digit prefix and writes a minimal MADR
/// template. `slugify` normalises the user-supplied title to a filename
/// form; collisions against existing ADRs fail loudly rather than
/// silently overwriting.
fn new_adr(title: &str) -> Result<()> {
    let adr_dir = PathBuf::from("docs/adr");
    if !adr_dir.is_dir() {
        bail!("ADR directory not found at {}", adr_dir.display());
    }

    let mut max_num: u32 = 0;
    for entry in fs::read_dir(&adr_dir).with_context(|| format!("reading {}", adr_dir.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(num_str) = name.split('-').next()
            && let Ok(n) = num_str.parse::<u32>()
        {
            max_num = max_num.max(n);
        }
    }

    let next_num = max_num + 1;
    let slug = slugify(title);
    if slug.is_empty() {
        bail!("title {title:?} produced an empty slug");
    }
    let filename = format!("{next_num:04}-{slug}.md");
    let path = adr_dir.join(&filename);

    if path.exists() {
        bail!("ADR already exists at {}", path.display());
    }

    let today = today_yyyy_mm_dd()?;
    let content = format!(
        "# {title}\n\
         \n\
         - **Status:** Proposed\n\
         - **Date:** {today}\n\
         \n\
         ## Context\n\n\
         ## Decision\n\n\
         ## Consequences\n",
    );

    fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    println!("created {}", path.display());
    Ok(())
}

fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn today_yyyy_mm_dd() -> Result<String> {
    let out = ProcessCommand::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .context("invoking `date` to stamp the new ADR")?;
    if !out.status.success() {
        bail!(
            "date command failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8(out.stdout)
        .context("date output was not valid UTF-8")?
        .trim()
        .to_owned())
}

fn deferred(subcmd: &str, hint: &str) -> anyhow::Error {
    let mut msg = format!(
        "`xtask {subcmd}` is declared for forward compatibility but not yet \
         implemented. A follow-up ADR tracks the design; for now perform the \
         operation manually. See CLAUDE.md for the interim workflow."
    );
    if !hint.is_empty() {
        msg.push_str(" [");
        msg.push_str(hint);
        msg.push(']');
    }
    anyhow!(msg)
}
