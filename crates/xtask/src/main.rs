//! Workspace automation.
//!
//! Every task invoked by the `Justfile` or by CI that isn't a direct
//! cargo / mdbook invocation lives here. Sub-commands:
//!
//! - `upstream-diff` — assert the vendored comrak tree is still
//!   pinned to the recorded SHA and that the ADR-0001 0-line diff
//!   budget is documented in `upstream/comrak/UPSTREAM_DIFF.md`.
//! - `upstream-sync` — replace `upstream/comrak/` with the source
//!   tree at a given upstream tag. Pure tree-replace (ADR-0001): the
//!   diff budget is 0, so there are no patches to re-apply.
//! - `new-adr` — scaffold a new MADR file under `docs/adr/`.
//! - `spec-refresh` — regenerate `spec/commonmark-*.json` /
//!   `spec/gfm-*.json` from cmark-format `spec.txt` inputs. Network
//!   fetching is handled by the `just spec-refresh` target
//!   (shell-side `curl`); this xtask only transforms
//!   already-downloaded spec files into fixture JSON.
//!
//! Aozora parser / corpus concerns live in the sibling `P4suta/aozora`
//! repo (ADR-0010), along with their refresh sub-commands.

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};

mod spec_refresh;
mod types;

/// ADR-0001 upstream diff budget, in lines. The vendored tree is verbatim
/// (no hooks), so the budget is 0; changing it requires a new ADR.
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
    /// upstream tag. Pure tree-replace (ADR-0001).
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
    /// Bump the `P4suta/aozora.git` rev pin to a new commit SHA across the
    /// workspace manifest (the umbrella `aozora` dep) and the cargo-fuzz
    /// crate in one pass, then refresh `Cargo.lock`. Keeping both pins on
    /// the same SHA stops a sync from silently leaving the fuzz crate
    /// behind the workspace.
    AozoraBump {
        /// Full 40-character lowercase hex commit SHA from `P4suta/aozora`'s
        /// `main` branch (or any other branch you intend to track).
        sha: String,
    },
    /// Generate the TypeScript `.d.ts` artefact for the IR + wasm
    /// envelope from the live Rust types, or drift-check it. The
    /// generated file (`crates/afm-wasm/types/afm_types.d.ts`) is the
    /// single source of truth for downstream TS consumers (playground,
    /// afm-obsidian); `types check` is the CI drift gate.
    Types(TypesArgs),
    /// Generate (or, with `--check`, drift-check) the release assets bundled
    /// into the dist archives: shell completions and the man page, written
    /// under `dist/assets/`. Shells out to the built `afm` binary so the CLI
    /// definition stays the single source of truth.
    GenDistAssets {
        /// Compare committed assets against fresh generation and exit non-zero
        /// on drift, instead of rewriting them.
        #[arg(long)]
        check: bool,
    },
}

#[derive(Args, Debug)]
struct TypesArgs {
    #[command(subcommand)]
    op: TypesOp,
}

#[derive(Subcommand, Debug)]
enum TypesOp {
    /// Regenerate `crates/afm-wasm/types/afm_types.d.ts` from the live
    /// IR + envelope types and write it. Overwrites the existing file;
    /// commit the diff.
    Ts,
    /// Compare the committed `afm_types.d.ts` against fresh codegen and
    /// exit non-zero on drift. Used as the CI gate so a renamed field /
    /// added variant forces the artefact regeneration step.
    Check,
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
        Command::Types(args) => types::dispatch(&args),
        Command::GenDistAssets { check } => gen_dist_assets(check),
    }
}

/// Shells (`clap_complete`) we ship completions for, with their conventional
/// install filenames.
const COMPLETION_TARGETS: [(&str, &str); 5] = [
    ("bash", "afm.bash"),
    ("zsh", "_afm"),
    ("fish", "afm.fish"),
    ("powershell", "_afm.ps1"),
    ("elvish", "afm.elv"),
];

/// Generate, or drift-check, the completion scripts and man page bundled into
/// the release archives. Generation runs the built `afm` binary so the CLI
/// definition is the single source of truth (afm-cli is a binary, not a
/// library, so xtask cannot import its `Cli` directly).
fn gen_dist_assets(check: bool) -> Result<()> {
    let afm = afm_binary_path();
    if !afm.is_file() {
        bail!(
            "gen-dist-assets: {} not found — build it first (`cargo build -p afm-cli`); \
             `just dist-assets` does this for you",
            afm.display()
        );
    }

    let comp_dir = PathBuf::from("dist/assets/completions");
    let man_path = PathBuf::from("dist/assets/man/afm.1");
    let mut drift: Vec<String> = Vec::new();

    for (shell, filename) in COMPLETION_TARGETS {
        let script = run_afm_capture(&afm, &["completions", shell])?;
        sync_or_check(&comp_dir.join(filename), &script, check, &mut drift)?;
    }
    let man = run_afm_capture(&afm, &["_man"])?;
    sync_or_check(&man_path, &man, check, &mut drift)?;

    if check {
        if drift.is_empty() {
            println!("gen-dist-assets: committed assets are up to date");
            Ok(())
        } else {
            bail!(
                "gen-dist-assets: {} asset(s) out of date ({}). \
                 Run `just dist-assets` and commit the result.",
                drift.len(),
                drift.join(", ")
            )
        }
    } else {
        println!(
            "gen-dist-assets: wrote {} completion script(s) + man page under dist/assets/",
            COMPLETION_TARGETS.len()
        );
        Ok(())
    }
}

/// Path to the debug `afm` binary, honoring `CARGO_TARGET_DIR`.
fn afm_binary_path() -> PathBuf {
    let target =
        env::var_os("CARGO_TARGET_DIR").map_or_else(|| PathBuf::from("target"), PathBuf::from);
    target.join("debug").join("afm")
}

/// Run the built `afm` binary with `args` and return its stdout, or bail on a
/// non-zero exit.
fn run_afm_capture(afm: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = ProcessCommand::new(afm)
        .args(args)
        .output()
        .with_context(|| format!("running {} {args:?}", afm.display()))?;
    if !out.status.success() {
        bail!(
            "{} {args:?} failed: {}",
            afm.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(out.stdout)
}

/// Write `content` to `dest` (creating parents), or in check mode record `dest`
/// as drifted when it differs.
fn sync_or_check(dest: &Path, content: &[u8], check: bool, drift: &mut Vec<String>) -> Result<()> {
    if check {
        let existing = fs::read(dest).unwrap_or_default();
        if existing != content {
            drift.push(dest.display().to_string());
        }
    } else {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
        }
        fs::write(dest, content).with_context(|| format!("writing {}", dest.display()))?;
    }
    Ok(())
}

/// Verify the ADR-0001 upstream-diff policy is in force.
///
/// Reads the pinned SHA + tag from `upstream/comrak/COMRAK_SHA` and
/// asserts that `upstream/comrak/UPSTREAM_DIFF.md` mentions the
/// current budget number ([`UPSTREAM_DIFF_BUDGET_LINES`]).
///
/// Byte-level enforcement against the upstream remote (network
/// fetch + diff) is **not** part of this gate: developers run
/// `cargo xtask upstream-sync <tag>` (a pure tree replace per ADR-0001)
/// to refresh the vendored tree, and any local modification
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
/// Pure tree-replace (ADR-0001): there are no afm patches to re-apply
/// because the diff budget is 0. We preserve the two
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
    // Render `0000-template.md` rather than hard-coding a divergent subset, so
    // a scaffolded ADR carries the same section set (Status / Date / Deciders /
    // Tags + Context / Decision / Consequences / Alternatives / References) the
    // committed ADRs use. The author fills Tags and the section bodies.
    let template_path = adr_dir.join("0000-template.md");
    let template = fs::read_to_string(&template_path)
        .with_context(|| format!("reading ADR template {}", template_path.display()))?;
    let content = template
        .replace("{{ADR NUMBER}}", &format!("{next_num:04}"))
        .replace("{{TITLE}}", title)
        .replace(
            "{proposed | accepted | deprecated | superseded by ADR-XXXX}",
            "proposed",
        )
        .replace("YYYY-MM-DD", &today);

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

/// Manifests carrying a `P4suta/aozora.git` rev pin. Post-B1 (PR #43) afm
/// depends on the single umbrella `aozora` crate — not the old six
/// internal crates — pinned in the workspace manifest; the
/// workspace-external cargo-fuzz crate pins the same rev independently.
/// `aozora-bump` rewrites both in one pass so a sync can't leave the fuzz
/// crate behind, then refreshes Cargo.lock. Idempotent: if every pin
/// already matches the target SHA, nothing is written and `cargo update`
/// is skipped.
const AOZORA_PINNED_MANIFESTS: [&str; 2] = ["Cargo.toml", "crates/afm-markdown/fuzz/Cargo.toml"];

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

    let pattern = regex::Regex::new(
        r#"(git = "https://github\.com/P4suta/aozora\.git", rev = ")([0-9a-f]{40})(")"#,
    )
    .expect("aozora rev pattern compiles");

    let mut total_found = 0_usize;
    let mut rewritten = 0_usize;
    for manifest in AOZORA_PINNED_MANIFESTS {
        let path = PathBuf::from(manifest);
        let original =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;

        let mut found = 0_usize;
        let mut already = 0_usize;
        let updated = pattern.replace_all(&original, |caps: &regex::Captures<'_>| {
            found += 1;
            if &caps[2] == new_sha {
                already += 1;
            }
            format!("{}{new_sha}{}", &caps[1], &caps[3])
        });

        // Each manifest pins the umbrella `aozora` exactly once. A
        // different count means the dep block was refactored — bail rather
        // than rewrite an unexpected shape and leave Cargo.lock inconsistent.
        if found != 1 {
            bail!(
                "aozora-bump: expected exactly one `P4suta/aozora.git` rev pin in {}, \
                 found {found}. The aozora dependency may have been refactored — update \
                 `AOZORA_PINNED_MANIFESTS` / the regex in xtask/src/main.rs and re-run.",
                path.display(),
            );
        }
        total_found += found;
        if already != found {
            fs::write(&path, updated.as_ref())
                .with_context(|| format!("writing {}", path.display()))?;
            rewritten += 1;
            println!("aozora-bump: rewrote {} to rev = {new_sha}", path.display());
        }
    }

    if rewritten == 0 {
        println!("aozora-bump: all {total_found} pins already at {new_sha}; no change.");
        return Ok(());
    }

    // Refresh the workspace Cargo.lock. The cargo-fuzz crate's lock is
    // git-ignored (regenerated on the next `cargo fuzz` build), so only the
    // umbrella `aozora` in the workspace needs an explicit update here.
    let status = ProcessCommand::new("cargo")
        .args(["update", "-p", "aozora"])
        .status()
        .context("invoking `cargo update -p aozora`")?;
    if !status.success() {
        bail!(
            "cargo update exited with {status:?}. The manifests were rewritten — \
             re-run `cargo update -p aozora` manually after fixing the fetch / \
             network issue."
        );
    }
    println!("aozora-bump: Cargo.lock refreshed against {new_sha}");
    Ok(())
}
