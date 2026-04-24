//! End-to-end integration tests for the `afm` CLI binary.
//!
//! Uses `CARGO_BIN_EXE_afm` (set by cargo for each `[[bin]]` target
//! during `cargo test`) so no `assert_cmd` dependency is pulled in.
//! Each test writes a temp file, invokes the binary with a specific
//! argument shape, and asserts on stdout / stderr / exit status.
//!
//! Coverage targets:
//!
//! - Default UTF-8 path: plain text, Aozora ruby, bracket annotations
//!   all render through the lexer + `post_process` pipeline and reach
//!   stdout.
//! - Shift_JIS path (`--encoding sjis`): the same pipeline accepts
//!   legacy Aozora .txt byte streams without pre-conversion.
//! - `check` subcommand: no-op on valid input, diagnostic on invalid.
//! - Help / version plumbing (clap); CLI must advertise itself.
//! - Failure modes: missing file, unreadable path, SJIS decode errors
//!   — each exits non-zero with a Japanese error message.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};
use std::str;
use std::time::{SystemTime, UNIX_EPOCH};

/// Absolute path to the freshly-built `afm` binary, provided by cargo
/// for every integration test in this crate.
fn afm_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_afm"))
}

/// Write `contents` to a new unique file under `/tmp`. Returns the
/// path; the file is cleaned up on test-process exit (we don't fuss
/// with explicit cleanup — tmp files are small and the test directory
/// is ephemeral inside the Docker sandbox).
fn write_temp_bytes(contents: &[u8], suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock went backward — filesystem tests cannot proceed")
        .subsec_nanos();
    let pid = process::id();
    // Include both pid and nanos so multiple tests running in parallel
    // don't collide on the same path.
    let path = env::temp_dir().join(format!("afm_cli_test_{pid}_{nanos}{suffix}"));
    let mut f = File::create(&path).expect("temp file create");
    f.write_all(contents).expect("temp file write");
    path
}

fn write_temp_utf8(contents: &str) -> PathBuf {
    write_temp_bytes(contents.as_bytes(), ".md")
}

fn run_afm(args: &[&str]) -> Output {
    Command::new(afm_bin())
        .args(args)
        .output()
        .expect("spawn afm binary")
}

fn stdout_of(out: &Output) -> &str {
    str::from_utf8(&out.stdout).expect("stdout must be UTF-8")
}

fn stderr_of(out: &Output) -> &str {
    str::from_utf8(&out.stderr).expect("stderr must be UTF-8")
}

// ---------------------------------------------------------------------------
// Help / version plumbing
// ---------------------------------------------------------------------------

#[test]
fn help_flag_succeeds_and_mentions_subcommands() {
    let out = run_afm(&["--help"]);
    assert!(
        out.status.success(),
        "--help must exit 0, stderr = {:?}",
        stderr_of(&out)
    );
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("render"),
        "--help must list `render` subcommand, got {stdout:?}"
    );
    assert!(
        stdout.contains("check"),
        "--help must list `check` subcommand, got {stdout:?}"
    );
}

#[test]
fn version_flag_succeeds_and_emits_non_empty_output() {
    let out = run_afm(&["--version"]);
    assert!(out.status.success(), "--version must exit 0");
    assert!(
        !stdout_of(&out).trim().is_empty(),
        "--version must print a non-empty version string"
    );
}

// ---------------------------------------------------------------------------
// `render` subcommand — UTF-8 path
// ---------------------------------------------------------------------------

#[test]
fn render_plain_utf8_to_html_on_stdout() {
    let path = write_temp_utf8("Hello, world.");
    let out = run_afm(&["render", path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "render plain UTF-8 must succeed, stderr = {:?}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).contains("<p>Hello, world.</p>"),
        "render output must wrap plain text in <p>, got {:?}",
        stdout_of(&out)
    );
}

#[test]
fn render_aozora_ruby_emits_ruby_tag() {
    let path = write_temp_utf8("｜青梅《おうめ》へ");
    let out = run_afm(&["render", path.to_str().unwrap()]);
    assert!(out.status.success(), "render must succeed");
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("<ruby>青梅"),
        "ruby recognition missing in CLI output, got {stdout:?}"
    );
    assert!(
        stdout.contains("<rt>おうめ"),
        "ruby reading missing, got {stdout:?}"
    );
}

#[test]
fn render_unknown_annotation_wraps_in_hidden_span() {
    let path = write_temp_utf8("前［＃ほげふが］後");
    let out = run_afm(&["render", path.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains(r#"<span class="afm-annotation" hidden>"#),
        "unknown ［＃...］ must wrap as afm-annotation span, got {stdout:?}"
    );
}

#[test]
fn render_page_break_emits_block_level_div() {
    let path = write_temp_utf8("前\n\n［＃改ページ］\n\n後");
    let out = run_afm(&["render", path.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains(r#"<div class="afm-page-break"></div>"#),
        "page break div missing, got {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// `render` subcommand — Shift_JIS path
// ---------------------------------------------------------------------------

#[test]
fn render_sjis_file_with_explicit_flag() {
    // 「青空文庫」 in Shift_JIS.
    let bytes = &[0x90, 0xC2, 0x8B, 0xF3, 0x95, 0xB6, 0x8C, 0xC9];
    let path = write_temp_bytes(bytes, ".txt");
    let out = run_afm(&["--encoding", "sjis", "render", path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "SJIS render must succeed, stderr = {:?}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).contains("青空文庫"),
        "decoded text must reach the HTML output, got {:?}",
        stdout_of(&out)
    );
}

#[test]
fn sjis_decode_failure_exits_nonzero_with_japanese_message() {
    // Invalid lead byte (0xFF) — SJIS decode must bail.
    let path = write_temp_bytes(&[0xFF, 0xFF, 0xFF], ".txt");
    let out = run_afm(&["--encoding", "sjis", "render", path.to_str().unwrap()]);
    assert!(
        !out.status.success(),
        "SJIS decode of invalid bytes must exit non-zero"
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("Shift_JIS"),
        "error must mention Shift_JIS, got {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// `check` subcommand
// ---------------------------------------------------------------------------

#[test]
fn check_succeeds_on_valid_utf8_input() {
    let path = write_temp_utf8("｜青梅《おうめ》");
    let out = run_afm(&["check", path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "check must exit 0 on valid input, stderr = {:?}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).is_empty(),
        "check must not emit on stdout, got {:?}",
        stdout_of(&out)
    );
}

#[test]
fn check_fails_on_undecodable_utf8() {
    // A raw 0x80 byte is not valid UTF-8 — read_input rejects it when
    // encoding=utf8 (default).
    let path = write_temp_bytes(&[0x80, 0x81], ".md");
    let out = run_afm(&["check", path.to_str().unwrap()]);
    assert!(!out.status.success(), "invalid UTF-8 must exit non-zero");
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("UTF-8"),
        "error must mention UTF-8, got {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// Failure modes
// ---------------------------------------------------------------------------

#[test]
fn render_missing_file_exits_nonzero_with_japanese_message() {
    let path = Path::new("/tmp/this_path_does_not_exist_ever_afm_cli_test");
    let out = run_afm(&["render", path.to_str().unwrap()]);
    assert!(!out.status.success(), "missing file must exit non-zero");
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("入力ファイル"),
        "error must be in Japanese and mention 入力ファイル, got {stderr:?}"
    );
}

#[test]
fn missing_subcommand_prints_usage_and_exits_nonzero() {
    let out = run_afm(&[]);
    assert!(
        !out.status.success(),
        "running with no args must fail (subcommand required)"
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("Usage") || stderr.contains("USAGE") || stderr.contains("使い方"),
        "no-args error must include usage hint, got {stderr:?}"
    );
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let out = run_afm(&["invalidsubcmd"]);
    assert!(
        !out.status.success(),
        "unknown subcommand must exit non-zero"
    );
}

// ---------------------------------------------------------------------------
// --strict + diagnostics flow
// ---------------------------------------------------------------------------

/// Craft an input the lexer *will* complain about. The Phase 2
/// balanced-stack walk flags an orphan `》` (ruby close with no
/// matching open) as `afm::lex::unmatched_close` — stable across
/// classifier iterations because the trigger table always pairs
/// `《` with `》`. If a future rewrite elides this shape silently,
/// the strict-mode test fails and forces the author to pick a
/// replacement canary that actually fires.
const DIAGNOSTIC_INPUT: &str = "orphan》close";

#[test]
fn check_without_strict_passes_even_with_diagnostics() {
    // Plain `check` must remain a syntax sanity-check that succeeds
    // on well-formed UTF-8 regardless of diagnostic count.
    let path = write_temp_utf8("hello");
    let out = run_afm(&["check", path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "plain check must exit 0, stderr = {:?}",
        stderr_of(&out)
    );
}

#[test]
fn render_strict_without_diagnostics_succeeds() {
    // --strict must NOT fail when the lexer produces zero diagnostics.
    let path = write_temp_utf8("clean input");
    let out = run_afm(&["--strict", "render", path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "strict render on clean input must exit 0, stderr = {:?}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).contains("<p>clean input</p>"),
        "render output must appear on stdout under strict mode, got {:?}",
        stdout_of(&out)
    );
}

/// When any lexer diagnostic fires under `--strict`, the binary
/// must:
///
/// * exit non-zero
/// * NOT print the HTML on stdout
/// * print a short Japanese error including the count
///
/// The diagnostic body itself lands on stderr via `emit_diagnostics`
/// so tooling can parse the `afm::…` code.
#[test]
fn render_strict_with_lexer_diagnostic_fails_nonzero() {
    let path = write_temp_utf8(DIAGNOSTIC_INPUT);
    let baseline = run_afm(&["check", path.to_str().unwrap()]);
    // Sanity: non-strict check must succeed (prove the diagnostic is
    // non-fatal in the default mode).
    assert!(
        baseline.status.success(),
        "non-strict check must still exit 0 on diagnostic-heavy input"
    );
    let strict = run_afm(&["--strict", "render", path.to_str().unwrap()]);
    assert!(
        !strict.status.success(),
        "--strict must turn the lexer diagnostic into a hard error"
    );
    let stderr = stderr_of(&strict);
    assert!(
        stderr.contains("--strict") || stderr.contains("診断"),
        "strict failure message must reference --strict or 診断, got {stderr:?}"
    );
}

#[test]
fn check_strict_on_diagnostic_input_fails() {
    let path = write_temp_utf8(DIAGNOSTIC_INPUT);
    let out = run_afm(&["--strict", "check", path.to_str().unwrap()]);
    assert!(
        !out.status.success(),
        "check --strict with lexer diagnostic must exit non-zero"
    );
}

#[test]
fn diagnostics_print_to_stderr_with_afm_code() {
    // Even without --strict, diagnostics surface on stderr so
    // tooling (Language servers, CI grep) can react.
    let path = write_temp_utf8(DIAGNOSTIC_INPUT);
    let out = run_afm(&["check", path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "check on diagnostic-heavy input must still exit 0 without --strict"
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("diagnostic [afm::"),
        "stderr must carry `diagnostic [afm::…]` lines, got {stderr:?}"
    );
}
