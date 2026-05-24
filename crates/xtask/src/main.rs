//! Workspace automation.
//!
//! Every task invoked by the `Justfile` or by CI that isn't a direct
//! cargo / mdbook invocation lives here. Sub-commands:
//!
//! - `upstream-diff` — assert the vendored comrak tree is still
//!   pinned to the recorded SHA and that the ADR-0001 0-line diff
//!   budget is documented in `upstream/comrak/UPSTREAM_DIFF.md`.
//! - `upstream-sync` — replace `upstream/comrak/` with the source
//!   tree at a given upstream tag. Pure tree-replace per ADR-0001
//!   v0.2.4: no patches to re-apply since the diff budget is 0.
//! - `new-adr` — scaffold a new MADR file under `docs/adr/`.
//! - `spec-refresh` — regenerate `spec/commonmark-*.json` /
//!   `spec/gfm-*.json` from cmark-format `spec.txt` inputs. Network
//!   fetching is handled by the `just spec-refresh` target
//!   (shell-side `curl`); this xtask only transforms
//!   already-downloaded spec files into fixture JSON.
//!
//! Aozora corpus refresh / Tier-A/B/C runs were once stubbed as
//! deferred sub-commands here. ADR-0010 (v0.2.0) moved every Aozora
//! parser / corpus concern into the sibling `P4suta/aozora` repo, so
//! those sub-commands now live there.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

mod spec_refresh;

/// ADR-0001 upstream diff budget, in lines. Changing this requires
/// a new ADR. Held at 200 from v0.1 through v0.2.3; collapsed to 0
/// in v0.2.4 (2026-04-30) — the historical patch surface was
/// removed once afm switched to HTML-sentinel post-processing
/// (ADR-0008). The budget value below tracks the v0.2.4 status.
const UPSTREAM_DIFF_BUDGET_LINES: usize = 0;

/// Upstream comrak repository URL. `upstream-sync` shallow-clones a
/// single tag from this remote.
const UPSTREAM_COMRAK_URL: &str = "https://github.com/kivikakk/comrak.git";

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
    /// Replace `upstream/comrak/` with the source tree at the given
    /// upstream tag. Pure tree-replace (ADR-0001 v0.2.4).
    UpstreamSync {
        /// Upstream tag name (e.g. `v0.53.0`).
        tag: String,
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
    /// Bump every `aozora-*` git dep in `Cargo.toml` to a new commit SHA in one
    /// pass, then refresh `Cargo.lock`. The six entries share a single upstream
    /// repo (`P4suta/aozora`); pinning them all to the same SHA keeps the
    /// borrowed-AST surface in lockstep across crates and prevents
    /// `cargo update` from silently advancing them one at a time.
    AozoraBump {
        /// Full 40-character lowercase hex commit SHA from `P4suta/aozora`'s
        /// `main` branch (or any other branch you intend to track).
        sha: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::UpstreamDiff => upstream_diff(),
        Command::UpstreamSync { tag } => upstream_sync(&tag),
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
        Command::AozoraBump { sha } => aozora_bump(&sha),
    }
}

/// Verify the ADR-0001 upstream-diff policy is in force.
///
/// Reads the pinned SHA + tag from `upstream/comrak/COMRAK_SHA` and
/// asserts that `upstream/comrak/UPSTREAM_DIFF.md` mentions the
/// current budget number ([`UPSTREAM_DIFF_BUDGET_LINES`]).
///
/// Byte-level enforcement against the upstream remote (network
/// fetch + diff) is **not** part of this gate: developers run
/// `cargo xtask upstream-sync <tag>` (a pure tree replace per ADR-0001
/// v0.2.4) to refresh the vendored tree, and any local modification
/// has to pass code review. The gate here catches accidental drift
/// in the policy file itself.
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

    // We want the budget number to appear in a phrase that names a
    // line count, not as a stray digit. `<n>-line` and `<n> lines`
    // are the two phrasings UPSTREAM_DIFF.md uses; either is enough.
    let needle_hyphen = format!("{UPSTREAM_DIFF_BUDGET_LINES}-line");
    let needle_word = format!("{UPSTREAM_DIFF_BUDGET_LINES} lines");
    if !diff_md.contains(&needle_hyphen) && !diff_md.contains(&needle_word) {
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

    Ok(())
}

/// Replace `upstream/comrak/` with the source tree at `tag`.
///
/// Pure tree-replace per ADR-0001 v0.2.4: there are no afm patches
/// to re-apply because the diff budget is 0. We preserve the two
/// afm-side metadata files (`COMRAK_SHA` and `UPSTREAM_DIFF.md`)
/// across the wipe, then rewrite `COMRAK_SHA` with the new pin.
///
/// Network: shells out to `git clone --depth 1 --branch <tag>`.
/// Run from a developer machine with internet access; CI does not
/// invoke this command.
fn upstream_sync(tag: &str) -> Result<()> {
    let upstream_dir = PathBuf::from("upstream/comrak");
    if !upstream_dir.is_dir() {
        bail!(
            "upstream-sync: {} not found; run from the workspace root",
            upstream_dir.display()
        );
    }

    let sha_path = upstream_dir.join("COMRAK_SHA");
    let diff_md_path = upstream_dir.join("UPSTREAM_DIFF.md");
    let preserved: Vec<(PathBuf, Vec<u8>)> = [&sha_path, &diff_md_path]
        .into_iter()
        .filter_map(|p| fs::read(p).ok().map(|c| (p.clone(), c)))
        .collect();

    let scratch = PathBuf::from("target/upstream-sync-tmp");
    if scratch.exists() {
        fs::remove_dir_all(&scratch)
            .with_context(|| format!("removing stale {}", scratch.display()))?;
    }
    if let Some(parent) = scratch.parent() {
        fs::create_dir_all(parent).with_context(|| format!("ensuring {}", parent.display()))?;
    }

    let status = ProcessCommand::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            tag,
            UPSTREAM_COMRAK_URL,
        ])
        .arg(&scratch)
        .status()
        .context("running `git clone`")?;
    if !status.success() {
        bail!("git clone failed for tag {tag:?}");
    }

    let sha_out = ProcessCommand::new("git")
        .arg("-C")
        .arg(&scratch)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("running `git rev-parse HEAD`")?;
    if !sha_out.status.success() {
        bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&sha_out.stderr)
        );
    }
    let sha = String::from_utf8(sha_out.stdout)
        .context("git rev-parse output not UTF-8")?
        .trim()
        .to_owned();

    // Drop the .git/ directory — we vendor the source tree, not a
    // working clone.
    let dot_git = scratch.join(".git");
    if dot_git.exists() {
        fs::remove_dir_all(&dot_git).with_context(|| format!("removing {}", dot_git.display()))?;
    }

    // Wipe and replace.
    fs::remove_dir_all(&upstream_dir)
        .with_context(|| format!("removing {}", upstream_dir.display()))?;
    copy_dir_recursive(&scratch, &upstream_dir)
        .with_context(|| format!("copying scratch tree into {}", upstream_dir.display()))?;

    // Restore afm metadata, then update COMRAK_SHA with the new pin.
    for (path, content) in preserved {
        fs::write(&path, content).with_context(|| format!("restoring {}", path.display()))?;
    }
    fs::write(&sha_path, format!("{sha}\n{tag}\n"))
        .with_context(|| format!("writing {}", sha_path.display()))?;

    fs::remove_dir_all(&scratch).with_context(|| format!("cleaning {}", scratch.display()))?;

    println!("upstream-sync: replaced upstream/comrak/ with comrak {tag} ({sha})");
    println!("upstream-sync: review the diff and run `just ci` before committing");
    Ok(())
}

/// Copy `src/` into `dst/` recursively. Mirrors the subset of
/// behaviour we need from a real `cp -R` for vendored source trees:
/// regular files are copied byte-for-byte, directories are
/// reconstructed, and symlinks fail loudly (comrak's tree has none,
/// and silently dropping them would break a future upstream change
/// without warning).
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("mkdir {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("read_dir {}", src.display()))? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if ty.is_symlink() {
            bail!(
                "unsupported symlink at {}; upstream comrak should only contain \
                 regular files and directories",
                from.display()
            );
        } else {
            // Regular file (or platform-specific kind we treat as a
            // file). `is_file()` would be too narrow on some
            // filesystems; `!is_dir() && !is_symlink()` handles
            // hardlinked entries that `fs::copy` accepts.
            fs::copy(&from, &to)
                .with_context(|| format!("copy {} -> {}", from.display(), to.display()))?;
        }
    }
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

/// The six `aozora-*` workspace deps (Cargo.toml) all point at
/// `P4suta/aozora.git` and share a single `rev = "<sha>"` pin (set
/// up in PR #27). This recipe rewrites all six pins in one pass and
/// runs `cargo update` against the same six packages so Cargo.lock
/// agrees. Idempotent: if the SHA is already current, the file is
/// left untouched and `cargo update` is skipped.
const AOZORA_CRATES: [&str; 6] = [
    "aozora-syntax",
    "aozora-pipeline",
    "aozora-render",
    "aozora-encoding",
    "aozora-spec",
    "aozora-proptest",
];

fn aozora_bump(new_sha: &str) -> Result<()> {
    // Accept only fully-spelled lowercase hex SHAs — short / mixed-case
    // SHAs would resolve fine via cargo update but make Cargo.toml diffs
    // harder to grep and Cargo.lock entries inconsistent.
    if new_sha.len() != 40
        || !new_sha
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    {
        bail!("aozora-bump: SHA must be exactly 40 lowercase hex characters, got: {new_sha:?}");
    }

    let cargo_toml = PathBuf::from("Cargo.toml");
    let original = fs::read_to_string(&cargo_toml)
        .with_context(|| format!("reading {}", cargo_toml.display()))?;

    let pattern = regex::Regex::new(
        r#"(git = "https://github\.com/P4suta/aozora\.git", rev = ")([0-9a-f]{40})(")"#,
    )
    .expect("aozora rev pattern compiles");

    let mut found = 0_usize;
    let mut already_current = 0_usize;
    let updated = pattern.replace_all(&original, |caps: &regex::Captures<'_>| {
        found += 1;
        if &caps[2] == new_sha {
            already_current += 1;
        }
        format!("{}{new_sha}{}", &caps[1], &caps[3])
    });

    if found == 0 {
        bail!(
            "aozora-bump: no `rev = \"...\"` entries pointing at P4suta/aozora.git \
             were found in {}. Has the workspace dep block been refactored?",
            cargo_toml.display(),
        );
    }
    if found != AOZORA_CRATES.len() {
        // Continuing with a partial match would rewrite some entries and
        // leave others on the old SHA, then `cargo update -p` would re-
        // sync Cargo.lock against the inconsistent state — every future
        // bump would silently inherit the drift. Bail instead, and let
        // the operator decide whether to update `AOZORA_CRATES` (if the
        // workspace dep block grew/shrank) or fix Cargo.toml.
        bail!(
            "aozora-bump: expected {} aozora entries pointing at \
             P4suta/aozora.git, found {found}. Cargo.toml may have been \
             refactored — update `AOZORA_CRATES` in xtask/src/main.rs \
             to match and re-run.",
            AOZORA_CRATES.len(),
        );
    }

    if already_current == found {
        println!("aozora-bump: all {found} entries already pinned to {new_sha}; no change.");
        return Ok(());
    }

    fs::write(&cargo_toml, updated.as_ref())
        .with_context(|| format!("writing {}", cargo_toml.display()))?;
    println!(
        "aozora-bump: rewrote {found} entries in {} to rev = {new_sha}",
        cargo_toml.display(),
    );

    // Refresh Cargo.lock. `-p <name>` per crate is more surgical than a
    // bare `cargo update` and matches the bump-comment instructions in
    // Cargo.toml itself.
    let mut update = ProcessCommand::new("cargo");
    update.arg("update");
    for crate_name in AOZORA_CRATES {
        update.args(["-p", crate_name]);
    }
    let status = update.status().context("invoking `cargo update`")?;
    if !status.success() {
        bail!(
            "cargo update exited with {status:?}. Cargo.toml was rewritten — \
             re-run `cargo update -p aozora-syntax …` manually after fixing \
             the underlying fetch / network issue."
        );
    }
    println!("aozora-bump: Cargo.lock refreshed against {new_sha}");
    Ok(())
}
