
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'afm' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'afm'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'afm' {
            [CompletionResult]::new('--encoding', '--encoding', [CompletionResultType]::ParameterName, 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'When to colorize diagnostics: auto (TTY-aware), always, or never')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Diagnostic output format: human-readable lines, or stable JSON for tooling')
            [CompletionResult]::new('--strict', '--strict', [CompletionResultType]::ParameterName, 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('render', 'render', [CompletionResultType]::ParameterValue, 'Render the input to HTML on stdout')
            [CompletionResult]::new('check', 'check', [CompletionResultType]::ParameterValue, 'Parse the input and report diagnostics without rendering')
            [CompletionResult]::new('completions', 'completions', [CompletionResultType]::ParameterValue, 'Generate a shell completion script on stdout')
            [CompletionResult]::new('_man', '_man', [CompletionResultType]::ParameterValue, 'Render the man page (roff) on stdout. Hidden; used by packaging')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'afm;render' {
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Write HTML here instead of stdout. Use `-` for stdout')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Write HTML here instead of stdout. Use `-` for stdout')
            [CompletionResult]::new('--encoding', '--encoding', [CompletionResultType]::ParameterName, 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'When to colorize diagnostics: auto (TTY-aware), always, or never')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Diagnostic output format: human-readable lines, or stable JSON for tooling')
            [CompletionResult]::new('--strict', '--strict', [CompletionResultType]::ParameterName, 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'afm;check' {
            [CompletionResult]::new('--encoding', '--encoding', [CompletionResultType]::ParameterName, 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'When to colorize diagnostics: auto (TTY-aware), always, or never')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Diagnostic output format: human-readable lines, or stable JSON for tooling')
            [CompletionResult]::new('--strict', '--strict', [CompletionResultType]::ParameterName, 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'afm;completions' {
            [CompletionResult]::new('--encoding', '--encoding', [CompletionResultType]::ParameterName, 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'When to colorize diagnostics: auto (TTY-aware), always, or never')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Diagnostic output format: human-readable lines, or stable JSON for tooling')
            [CompletionResult]::new('--strict', '--strict', [CompletionResultType]::ParameterName, 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'afm;_man' {
            [CompletionResult]::new('--encoding', '--encoding', [CompletionResultType]::ParameterName, 'Input encoding. Defaults to UTF-8; use `sjis` for raw Aozora Bunko files')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'When to colorize diagnostics: auto (TTY-aware), always, or never')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Diagnostic output format: human-readable lines, or stable JSON for tooling')
            [CompletionResult]::new('--strict', '--strict', [CompletionResultType]::ParameterName, 'Treat any lexer/parser diagnostic as a hard error (exit 2). Default: warn and pass through')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Increase log verbosity (-v info, -vv debug, -vvv trace). `RUST_LOG` overrides')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Decrease log verbosity (-q errors only). `RUST_LOG` overrides')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'afm;help' {
            [CompletionResult]::new('render', 'render', [CompletionResultType]::ParameterValue, 'Render the input to HTML on stdout')
            [CompletionResult]::new('check', 'check', [CompletionResultType]::ParameterValue, 'Parse the input and report diagnostics without rendering')
            [CompletionResult]::new('completions', 'completions', [CompletionResultType]::ParameterValue, 'Generate a shell completion script on stdout')
            [CompletionResult]::new('_man', '_man', [CompletionResultType]::ParameterValue, 'Render the man page (roff) on stdout. Hidden; used by packaging')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'afm;help;render' {
            break
        }
        'afm;help;check' {
            break
        }
        'afm;help;completions' {
            break
        }
        'afm;help;_man' {
            break
        }
        'afm;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
