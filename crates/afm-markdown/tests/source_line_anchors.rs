//! End-to-end tests for the source-line anchor option.

use afm_markdown::{Options, render_to_string};

#[test]
fn anchors_emit_only_when_option_is_on() {
    let src = "para line 1\n\npara line 3\n";
    let off = render_to_string(src, &Options::afm_default());
    let on = render_to_string(src, &Options::afm_default().with_source_line_anchors(true));
    assert!(!off.html.contains("data-afm-source-line"));
    assert!(on.html.contains(r#"<p data-afm-source-line=""#));
}

#[test]
fn anchors_are_one_based() {
    let on = render_to_string(
        "first\n\nsecond\n",
        &Options::afm_default().with_source_line_anchors(true),
    );
    assert!(on.html.contains(r#"data-afm-source-line="1""#));
    assert!(on.html.contains(r#"data-afm-source-line="3""#));
}

#[test]
fn anchors_apply_to_headings() {
    let on = render_to_string(
        "# Title\n\nbody\n",
        &Options::afm_default().with_source_line_anchors(true),
    );
    assert!(on.html.contains(r#"<h1 data-afm-source-line="1""#));
    assert!(on.html.contains(r#"<p data-afm-source-line="3""#));
}

#[test]
fn anchors_not_applied_when_aozora_disabled_and_option_off() {
    // commonmark_only() has source_line_anchors: false. Adding the
    // builder afterwards should still flip the bit.
    let on = render_to_string(
        "p\n",
        &Options::commonmark_only().with_source_line_anchors(true),
    );
    assert!(on.html.contains(r#"<p data-afm-source-line=""#));
}
