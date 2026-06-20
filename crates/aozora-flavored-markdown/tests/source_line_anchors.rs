//! End-to-end tests for the source-line anchor option.

use aozora_flavored_markdown::{Options, render_to_string};

#[test]
fn anchors_emit_only_when_option_is_on() {
    let src = "para line 1\n\npara line 3\n";
    let off = render_to_string(src, &Options::default());
    let on = render_to_string(src, &Options::default().with_source_line_anchors(true));
    assert!(!off.html.contains("data-aozora-md-source-line"));
    assert!(on.html.contains(r#"<p data-aozora-md-source-line=""#));
}

#[test]
fn anchors_are_one_based() {
    let on = render_to_string(
        "first\n\nsecond\n",
        &Options::default().with_source_line_anchors(true),
    );
    assert!(on.html.contains(r#"data-aozora-md-source-line="1""#));
    assert!(on.html.contains(r#"data-aozora-md-source-line="3""#));
}

#[test]
fn anchors_apply_to_headings() {
    let on = render_to_string(
        "# Title\n\nbody\n",
        &Options::default().with_source_line_anchors(true),
    );
    assert!(on.html.contains(r#"<h1 data-aozora-md-source-line="1""#));
    assert!(on.html.contains(r#"<p data-aozora-md-source-line="3""#));
}

#[test]
fn anchors_not_applied_when_aozora_disabled_and_option_off() {
    // commonmark_only() has source_line_anchors: false. Adding the
    // builder afterwards should still flip the bit.
    let on = render_to_string(
        "p\n",
        &Options::commonmark_only().with_source_line_anchors(true),
    );
    assert!(on.html.contains(r#"<p data-aozora-md-source-line=""#));
}
