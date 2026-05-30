//! `afm` — command-line interface.
//!
//! Sub-commands:
//!   - `afm render <input>`: render an afm (.md) file to HTML on stdout.
//!   - `afm check <input>`:  parse and surface diagnostics only; no rendering.
//!
//! Input files may be UTF-8 (default) or Shift_JIS (with `--encoding sjis`) to read
//! original Aozora Bunko .txt distributions without pre-conversion.

#![forbid(unsafe_code)]

use std::io;
use std::path::{Path, PathBuf};
use std::{fs, process::ExitCode};

use afm_markdown::{Options, render_to_string};
use aozora::encoding::decode_sjis;
use clap::{Parser, Subcommand, ValueEnum};
use miette::{IntoDiagnostic, Result, WrapErr};

#[derive(Parser, Debug)]
#[command(
    name = "afm",
    version,
    about = "aozora-flavored-markdown CLI",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files.
    #[arg(long, global = true, value_enum, default_value_t = InputEncoding::Utf8)]
    encoding: InputEncoding,

    /// Treat any unknown annotation as a hard error (default: warn and pass through).
    #[arg(long, global = true)]
    strict: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Render the input to HTML on stdout.
    Render {
        /// Path to the afm source. Use `-` for stdin.
        input: PathBuf,
    },
    /// Parse the input and report diagnostics without rendering.
    Check { input: PathBuf },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum InputEncoding {
    Utf8,
    Sjis,
}

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("{err:?}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Render { input } => render(&input, cli.encoding, cli.strict),
        Command::Check { input } => check(&input, cli.encoding, cli.strict),
    }
}

fn read_input(path: &Path, encoding: InputEncoding) -> Result<String> {
    let bytes = fs::read(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("入力ファイルを読めません: {}", path.display()))?;
    match encoding {
        InputEncoding::Utf8 => String::from_utf8(bytes)
            .into_diagnostic()
            .wrap_err("UTF-8 としてデコードできません — --encoding sjis を試してください"),
        InputEncoding::Sjis => decode_sjis(&bytes).map_err(Into::into),
    }
}

fn render(path: &Path, encoding: InputEncoding, strict: bool) -> Result<()> {
    let source = read_input(path, encoding)?;
    let options = Options::afm_default();
    let result = render_to_string(&source, &options);
    emit_diagnostics(&result.diagnostics);
    if strict && !result.diagnostics.is_empty() {
        return Err(miette::miette!(
            "lexer が {} 件の診断を報告しました (--strict)",
            result.diagnostics.len()
        ));
    }
    println!("{}", result.html);
    Ok(())
}

fn check(path: &Path, encoding: InputEncoding, strict: bool) -> Result<()> {
    let source = read_input(path, encoding)?;
    let options = Options::afm_default();
    let result = render_to_string(&source, &options);
    emit_diagnostics(&result.diagnostics);
    if strict && !result.diagnostics.is_empty() {
        return Err(miette::miette!(
            "lexer が {} 件の診断を報告しました (--strict)",
            result.diagnostics.len()
        ));
    }
    Ok(())
}

/// Print every diagnostic on stderr with its miette-derived code so
/// downstream tooling (language servers, CI gates, LSP JSON bridges)
/// can key on the stable `afm::…` strings rather than free-form
/// messages.
fn emit_diagnostics(diagnostics: &[afm_markdown::Diagnostic]) {
    for d in diagnostics {
        eprintln!("diagnostic [{}]: {d}", d.code());
    }
}
