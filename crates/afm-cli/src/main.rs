//! `afm` — command-line interface.
//!
//! Sub-commands:
//!   - `afm render <input>`: render an afm (.md) file to HTML on stdout.
//!   - `afm check <input>`:  parse and surface diagnostics only; no rendering.
//!
//! `<input>` is a file path, or `-` to read from standard input. Input bytes may
//! be UTF-8 (default) or Shift_JIS (with `--encoding sjis`) to read original
//! Aozora Bunko .txt distributions without pre-conversion.

#![forbid(unsafe_code)]

use std::io::{self, Read};
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

    /// Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through.
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
    Check {
        /// Path to the afm source. Use `-` for stdin.
        input: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum InputEncoding {
    Utf8,
    Sjis,
}

/// Resolved inputs for one render/check pass. Carrying these in a struct
/// (rather than a fistful of positional args) keeps the shared pipeline under
/// clippy's argument-count and bool-parameter limits as later phases add flags.
#[derive(Debug)]
struct PipelineArgs {
    input: PathBuf,
    encoding: InputEncoding,
    strict: bool,
    /// `render` prints HTML on success; `check` parses only.
    emit_html: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:?}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(io::stderr)
        .init();

    let cli = Cli::parse();

    let args = match cli.command {
        Command::Render { input } => PipelineArgs {
            input,
            encoding: cli.encoding,
            strict: cli.strict,
            emit_html: true,
        },
        Command::Check { input } => PipelineArgs {
            input,
            encoding: cli.encoding,
            strict: cli.strict,
            emit_html: false,
        },
    };
    run_pipeline(&args)
}

/// Read → render → report. Shared by `render` and `check`; the only difference
/// is whether HTML reaches stdout on success. Returns exit code 2 when
/// `--strict` promotes a lexer diagnostic to an error, otherwise 0.
fn run_pipeline(args: &PipelineArgs) -> Result<ExitCode> {
    let source = read_input(&args.input, args.encoding)?;
    let options = Options::afm_default();
    let result = render_to_string(&source, &options);
    emit_diagnostics(&result.diagnostics);

    if args.strict && !result.diagnostics.is_empty() {
        eprintln!(
            "lexer が {} 件の診断を報告しました (--strict)",
            result.diagnostics.len()
        );
        return Ok(ExitCode::from(2));
    }

    if args.emit_html {
        println!("{}", result.html);
    }
    Ok(ExitCode::SUCCESS)
}

/// Read the input as raw bytes — from a file path, or from standard input when
/// `input` is `-`. Encoding-agnostic; `read_input` performs the decode.
fn read_bytes(input: &Path) -> Result<Vec<u8>> {
    if input == Path::new("-") {
        let mut buf = Vec::new();
        io::stdin()
            .lock()
            .read_to_end(&mut buf)
            .into_diagnostic()
            .wrap_err("標準入力を読めません")?;
        Ok(buf)
    } else {
        fs::read(input)
            .into_diagnostic()
            .wrap_err_with(|| format!("入力ファイルを読めません: {}", input.display()))
    }
}

fn read_input(input: &Path, encoding: InputEncoding) -> Result<String> {
    let bytes = read_bytes(input)?;
    match encoding {
        InputEncoding::Utf8 => String::from_utf8(bytes)
            .into_diagnostic()
            .wrap_err("UTF-8 としてデコードできません — --encoding sjis を試してください"),
        InputEncoding::Sjis => decode_sjis(&bytes).map_err(Into::into),
    }
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
