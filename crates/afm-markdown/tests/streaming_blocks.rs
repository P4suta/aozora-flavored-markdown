//! Tests for the per-block streaming render API
//! (`render_blocks_to_ir`).

use afm_markdown::ir::IrBlock;
use afm_markdown::{Options, render_blocks_to_ir};

#[test]
fn empty_input_yields_no_blocks() {
    let (blocks, diagnostics) = render_blocks_to_ir("", &Options::afm_default());
    assert!(blocks.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn each_top_level_block_yields_one_rendered_block() {
    let src = "first\n\nsecond\n\nthird\n";
    let (blocks, _) = render_blocks_to_ir(src, &Options::afm_default());
    assert_eq!(blocks.len(), 3);
    // Each rendered block has its own HTML chunk.
    for block in &blocks {
        assert!(block.html.starts_with("<p>"));
    }
}

#[test]
fn block_source_lines_are_one_based() {
    let src = "a\n\nb\n\nc\n";
    let (blocks, _) = render_blocks_to_ir(src, &Options::afm_default());
    assert_eq!(blocks[0].source_line, 1);
    assert_eq!(blocks[1].source_line, 3);
    assert_eq!(blocks[2].source_line, 5);
}

#[test]
fn aozora_inline_renders_inside_per_block_html() {
    let src = "｜漢字《かんじ》\n\nplain second\n";
    let (blocks, diagnostics) = render_blocks_to_ir(src, &Options::afm_default());
    assert!(diagnostics.is_empty());
    assert_eq!(blocks.len(), 2);
    assert!(blocks[0].html.contains("<ruby>"));
    assert!(!blocks[0].html.contains("｜"));
    assert!(!blocks[1].html.contains("<ruby>"));
}

#[test]
fn aozora_disabled_path_skips_lex_pre_pass() {
    let src = "first\n\nsecond\n";
    let mut opts = Options::commonmark_only();
    opts.aozora_enabled = false;
    let (blocks, diagnostics) = render_blocks_to_ir(src, &opts);
    assert_eq!(blocks.len(), 2);
    assert!(diagnostics.is_empty());
}

#[test]
fn heading_blocks_carry_their_kind_in_ir() {
    let src = "# Title\n\nbody\n";
    let (blocks, _) = render_blocks_to_ir(src, &Options::afm_default());
    assert_eq!(blocks.len(), 2);
    let kind = match blocks[0].ir.first() {
        Some(IrBlock::Heading { level, .. }) => Some(*level),
        _ => None,
    };
    assert_eq!(kind, Some(1));
}
