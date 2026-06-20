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
//!   all render through the lexer + splice pipeline and reach stdout.
//! - Shift_JIS path (`--encoding sjis`): the same pipeline accepts
//!   legacy Aozora .txt byte streams without pre-conversion.
//! - `check` subcommand: no-op on valid input, diagnostic on invalid.
//! - Help / version plumbing (clap); CLI must advertise itself.
//! - Failure modes: missing file, unreadable path, SJIS decode errors
//!   — each exits non-zero with a Japanese error message.

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output, Stdio};
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

/// Like `run_afm` but feeds `stdin` to the child's standard input, so we can
/// exercise the `-` (read-from-stdin) input path. The piped `ChildStdin` is
/// dropped right after the write, closing the pipe so the child sees EOF; our
/// inputs are tiny, so writing before reading cannot deadlock the OS buffer.
fn run_afm_stdin(args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(afm_bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn afm binary");
    child
        .stdin
        .take()
        .expect("child stdin is piped")
        .write_all(stdin)
        .expect("write child stdin");
    child.wait_with_output().expect("wait for afm binary")
}

/// Run `afm` with a hermetic environment: the colour-related vars that
/// otherwise leak in from CI are cleared, then `envs` is applied. Lets the
/// `--color` / `NO_COLOR` / `CLICOLOR_FORCE` tests assert on a known baseline.
fn run_afm_env(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(afm_bin());
    cmd.args(args)
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("RUST_LOG");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().expect("spawn afm binary")
}

/// A unique, not-yet-created path under the temp dir — for `-o <file>` tests.
fn unique_temp_path(suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock went backward")
        .subsec_nanos();
    env::temp_dir().join(format!("afm_cli_out_{}_{nanos}{suffix}", process::id()))
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

#[test]
fn help_strict_text_matches_behavior() {
    // The `--strict` help once claimed it only fired on "unknown
    // annotation", but the flag promotes *any* lexer diagnostic. Guard
    // against the stale wording creeping back, and assert the corrected
    // text mentions diagnostics.
    let out = run_afm(&["--help"]);
    assert!(out.status.success(), "--help must exit 0");
    let stdout = stdout_of(&out);
    assert!(
        !stdout.contains("annotation"),
        "--strict help must not claim 'unknown annotation', got {stdout:?}"
    );
    assert!(
        stdout.contains("diagnostic"),
        "--strict help must describe diagnostics, got {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// stdin (`-`) input path
// ---------------------------------------------------------------------------

#[test]
fn render_reads_from_stdin_dash() {
    let out = run_afm_stdin(&["render", "-"], b"Hello, world.");
    assert!(
        out.status.success(),
        "render from stdin must exit 0, stderr = {:?}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).contains("<p>Hello, world.</p>"),
        "stdin body must render to <p>, got {:?}",
        stdout_of(&out)
    );
}

#[test]
fn render_stdin_sjis_decodes() {
    // 「青空文庫」 in Shift_JIS — same bytes as the file-based SJIS test,
    // proving `--encoding sjis` also applies to the stdin byte stream.
    let bytes = &[0x90, 0xC2, 0x8B, 0xF3, 0x95, 0xB6, 0x8C, 0xC9];
    let out = run_afm_stdin(&["--encoding", "sjis", "render", "-"], bytes);
    assert!(
        out.status.success(),
        "SJIS stdin render must succeed, stderr = {:?}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).contains("青空文庫"),
        "decoded stdin text must reach the HTML output, got {:?}",
        stdout_of(&out)
    );
}

#[test]
fn check_reads_from_stdin_dash() {
    let out = run_afm_stdin(&["check", "-"], "｜青梅《おうめ》".as_bytes());
    assert!(
        out.status.success(),
        "check from stdin must exit 0, stderr = {:?}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).is_empty(),
        "check must not emit on stdout, got {:?}",
        stdout_of(&out)
    );
}

#[test]
fn render_stdin_dash_undecodable_utf8_fails() {
    let out = run_afm_stdin(&["render", "-"], &[0x80, 0x81]);
    assert!(
        !out.status.success(),
        "invalid UTF-8 on stdin must exit non-zero"
    );
    assert!(
        stderr_of(&out).contains("UTF-8"),
        "error must mention UTF-8, got {:?}",
        stderr_of(&out)
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

// ---------------------------------------------------------------------------
// Exit-code contract: 0 success / 1 generic error / 2 strict diagnostic
// ---------------------------------------------------------------------------

#[test]
fn strict_diagnostic_exits_with_code_two() {
    // `ref/cli.md` promises exit code 2 specifically for a strict-mode
    // diagnostic, distinct from generic failures (code 1).
    let path = write_temp_utf8(DIAGNOSTIC_INPUT);
    let out = run_afm(&["--strict", "render", path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "strict diagnostic must exit with code 2, stderr = {:?}",
        stderr_of(&out)
    );
}

#[test]
fn check_strict_diagnostic_exits_code_two() {
    let path = write_temp_utf8(DIAGNOSTIC_INPUT);
    let out = run_afm(&["--strict", "check", path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "check --strict diagnostic must exit with code 2, stderr = {:?}",
        stderr_of(&out)
    );
}

#[test]
fn generic_error_exits_with_code_one() {
    // A plain I/O failure must stay code 1 so it is distinguishable from
    // the strict-diagnostic code 2.
    let out = run_afm(&["render", "/tmp/this_path_does_not_exist_ever_afm_cli_test"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "generic I/O error must exit with code 1, stderr = {:?}",
        stderr_of(&out)
    );
}

#[test]
fn diagnostics_print_to_stderr_with_aozora_code() {
    // Even without --strict, diagnostics surface on stderr so tooling
    // (Language servers, CI grep) can react. The lexer lives in the
    // sibling `aozora` crate, so the diagnostic codes carry the
    // `aozora::…` prefix from upstream.
    let path = write_temp_utf8(DIAGNOSTIC_INPUT);
    let out = run_afm(&["check", path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "check on diagnostic-heavy input must still exit 0 without --strict"
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("diagnostic [aozora::"),
        "stderr must carry `diagnostic [aozora::…]` lines, got {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// `--output` / `-o`
// ---------------------------------------------------------------------------

#[test]
fn render_output_file_writes_html() {
    let src = write_temp_utf8("Hello, world.");
    let dst = unique_temp_path(".html");
    let out = run_afm(&["render", src.to_str().unwrap(), "-o", dst.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "render -o must succeed, stderr = {:?}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).is_empty(),
        "render -o <file> must not also print HTML to stdout, got {:?}",
        stdout_of(&out)
    );
    let written = fs::read_to_string(&dst).expect("output file must exist");
    assert!(
        written.contains("<p>Hello, world.</p>"),
        "output file must contain the rendered HTML, got {written:?}"
    );
}

#[test]
fn render_output_dash_is_stdout() {
    let src = write_temp_utf8("Hello, world.");
    let out = run_afm(&["render", src.to_str().unwrap(), "-o", "-"]);
    assert!(out.status.success(), "render -o - must succeed");
    assert!(
        stdout_of(&out).contains("<p>Hello, world.</p>"),
        "-o - must write HTML to stdout, got {:?}",
        stdout_of(&out)
    );
}

#[test]
fn render_output_to_unwritable_path_fails() {
    let src = write_temp_utf8("Hello, world.");
    // The parent directory does not exist, so the file cannot be created.
    let out = run_afm(&[
        "render",
        src.to_str().unwrap(),
        "-o",
        "/tmp/afm_no_such_dir_xyz/out.html",
    ]);
    assert!(
        !out.status.success(),
        "unwritable -o path must exit non-zero"
    );
    assert!(
        stderr_of(&out).contains("出力ファイル"),
        "error must mention 出力ファイル, got {:?}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------------------
// `--color` / NO_COLOR / CLICOLOR_FORCE
// ---------------------------------------------------------------------------

/// A non-existent input drives the miette error report, which is where the
/// colour choice is observable.
const MISSING_INPUT: &str = "/tmp/this_path_does_not_exist_ever_afm_cli_color";

/// ANSI escape introducer (ESC).
const ESC: char = '\u{1b}';

#[test]
fn color_always_emits_ansi_even_when_piped() {
    let out = run_afm_env(&["--color", "always", "render", MISSING_INPUT], &[]);
    assert!(!out.status.success(), "missing file must still fail");
    assert!(
        stderr_of(&out).contains(ESC),
        "--color always must emit ANSI, got {:?}",
        stderr_of(&out)
    );
}

#[test]
fn color_never_suppresses_ansi() {
    let out = run_afm_env(&["--color", "never", "render", MISSING_INPUT], &[]);
    assert!(!out.status.success());
    assert!(
        !stderr_of(&out).contains(ESC),
        "--color never must not emit ANSI, got {:?}",
        stderr_of(&out)
    );
}

#[test]
fn no_color_env_disables_color() {
    let out = run_afm_env(&["render", MISSING_INPUT], &[("NO_COLOR", "1")]);
    assert!(!out.status.success());
    assert!(
        !stderr_of(&out).contains(ESC),
        "NO_COLOR must disable ANSI under the default auto mode, got {:?}",
        stderr_of(&out)
    );
}

#[test]
fn clicolor_force_enables_color() {
    let out = run_afm_env(&["render", MISSING_INPUT], &[("CLICOLOR_FORCE", "1")]);
    assert!(!out.status.success());
    assert!(
        stderr_of(&out).contains(ESC),
        "CLICOLOR_FORCE must enable ANSI even when stderr is piped, got {:?}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------------------
// `-v` / `-q` verbosity plumbing
// ---------------------------------------------------------------------------

#[test]
fn verbose_and_quiet_flags_parse_and_run() {
    let src = write_temp_utf8("clean input");
    let path = src.to_str().unwrap();
    let cases: [&[&str]; 4] = [&["-v"], &["-vvv"], &["-q"], &["-qq"]];
    for flag in cases {
        let mut args: Vec<&str> = flag.to_vec();
        args.push("render");
        args.push(path);
        let out = run_afm(&args);
        assert!(
            out.status.success(),
            "{flag:?} render must exit 0, stderr = {:?}",
            stderr_of(&out)
        );
        assert!(
            stdout_of(&out).contains("<p>clean input</p>"),
            "{flag:?} must still render to stdout, got {:?}",
            stdout_of(&out)
        );
    }
}
