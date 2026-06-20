
use builtin;
use str;

set edit:completion:arg-completer[afm] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'afm'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'afm'= {
            cand --encoding 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files'
            cand --color 'When to colorize diagnostics: auto (TTY-aware), always, or never'
            cand --format 'Diagnostic output format: human-readable lines, or stable JSON for tooling'
            cand --strict 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through'
            cand -v 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand --verbose 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand -q 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand --quiet 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
            cand render 'Render the input to HTML on stdout'
            cand check 'Parse the input and report diagnostics without rendering'
            cand completions 'Generate a shell completion script on stdout'
            cand _man 'Render the man page (roff) on stdout. Hidden; used by packaging'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'afm;render'= {
            cand -o 'Write HTML here instead of stdout. Use `-` for stdout'
            cand --output 'Write HTML here instead of stdout. Use `-` for stdout'
            cand --encoding 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files'
            cand --color 'When to colorize diagnostics: auto (TTY-aware), always, or never'
            cand --format 'Diagnostic output format: human-readable lines, or stable JSON for tooling'
            cand --strict 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through'
            cand -v 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand --verbose 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand -q 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand --quiet 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'afm;check'= {
            cand --encoding 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files'
            cand --color 'When to colorize diagnostics: auto (TTY-aware), always, or never'
            cand --format 'Diagnostic output format: human-readable lines, or stable JSON for tooling'
            cand --strict 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through'
            cand -v 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand --verbose 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand -q 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand --quiet 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'afm;completions'= {
            cand --encoding 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files'
            cand --color 'When to colorize diagnostics: auto (TTY-aware), always, or never'
            cand --format 'Diagnostic output format: human-readable lines, or stable JSON for tooling'
            cand --strict 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through'
            cand -v 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand --verbose 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand -q 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand --quiet 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'afm;_man'= {
            cand --encoding 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files'
            cand --color 'When to colorize diagnostics: auto (TTY-aware), always, or never'
            cand --format 'Diagnostic output format: human-readable lines, or stable JSON for tooling'
            cand --strict 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through'
            cand -v 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand --verbose 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides'
            cand -q 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand --quiet 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'afm;help'= {
            cand render 'Render the input to HTML on stdout'
            cand check 'Parse the input and report diagnostics without rendering'
            cand completions 'Generate a shell completion script on stdout'
            cand _man 'Render the man page (roff) on stdout. Hidden; used by packaging'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'afm;help;render'= {
        }
        &'afm;help;check'= {
        }
        &'afm;help;completions'= {
        }
        &'afm;help;_man'= {
        }
        &'afm;help;help'= {
        }
    ]
    $completions[$command]
}
