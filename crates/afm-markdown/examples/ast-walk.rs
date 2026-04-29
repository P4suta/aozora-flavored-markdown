//! Walk the lexer's borrowed-AST registry and report how often each
//! `AozoraNode` variant appears, plus the number of lexer diagnostics
//! for the input.
//!
//! Run:
//!
//!     cargo run --example ast-walk -p afm-markdown -- input.md

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::process::ExitCode;

use afm_markdown::{
    BLOCK_CLOSE_SENTINEL, BLOCK_LEAF_SENTINEL, BLOCK_OPEN_SENTINEL, INLINE_SENTINEL,
};
use aozora_lex::lex_into_arena;
use aozora_syntax::borrowed::{AozoraNode, Arena, NodeRef};

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: ast-walk <path/to/input.md>");
        return ExitCode::from(2);
    };

    let input = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let arena = Arena::new();
    let lex_out = lex_into_arena(&input, &arena);

    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (idx, ch) in lex_out.normalized.char_indices() {
        let is_sentinel = matches!(
            ch,
            INLINE_SENTINEL | BLOCK_LEAF_SENTINEL | BLOCK_OPEN_SENTINEL | BLOCK_CLOSE_SENTINEL
        );
        if !is_sentinel {
            continue;
        }
        let pos = u32::try_from(idx).expect("normalized text fits u32");
        let Some(node_ref) = lex_out.registry.node_at(pos) else {
            continue;
        };
        let kind = match node_ref {
            NodeRef::BlockOpen(_) => "Container(open)",
            NodeRef::BlockClose(_) => "Container(close)",
            NodeRef::BlockLeaf(node) | NodeRef::Inline(node) => match node {
                AozoraNode::Ruby(_) => "Ruby",
                AozoraNode::Bouten(_) => "Bouten",
                AozoraNode::TateChuYoko(_) => "TateChuYoko",
                AozoraNode::Gaiji(_) => "Gaiji",
                AozoraNode::Annotation(_) => "Annotation",
                AozoraNode::Kaeriten(_) => "Kaeriten",
                AozoraNode::DoubleRuby(_) => "DoubleRuby",
                AozoraNode::Sashie(_) => "Sashie",
                AozoraNode::AozoraHeading(_) => "AozoraHeading",
                AozoraNode::HeadingHint(_) => "HeadingHint",
                AozoraNode::Indent(_) => "Indent",
                AozoraNode::AlignEnd(_) => "AlignEnd",
                AozoraNode::PageBreak => "PageBreak",
                AozoraNode::SectionBreak(_) => "SectionBreak",
                _ => "Other",
            },
            _ => "Other(noderef)",
        };
        *counts.entry(kind).or_insert(0) += 1;
    }

    let width = counts
        .values()
        .copied()
        .max()
        .unwrap_or(0)
        .to_string()
        .len()
        .max(1);
    for (kind, n) in &counts {
        println!("{n:>width$}  {kind}");
    }
    let diag_count = lex_out.diagnostics.len();
    println!("{diag_count:>width$}  lexer diagnostics");
    ExitCode::SUCCESS
}
