//! Source-line anchor injection for the HTML renderer.
//!
//! When `Options::source_line_anchors` is `true`, top-level block
//! elements in the rendered HTML get a `data-afm-source-line="N"`
//! attribute (1-based) pointing back at the source line where the
//! block began. The afm-obsidian document-mode adapter (Pillar 6)
//! relies on this to map Obsidian's per-block post-processor calls
//! back to slices of the full rendered fragment.
//!
//! Algorithm:
//!
//! 1. Walk comrak's top-level AST children (`root.children()`) and
//!    collect each child's source position from
//!    `node.data.borrow().sourcepos.start.line`.
//! 2. After comrak emits the HTML string, scan it once and inject
//!    `data-afm-source-line="N"` into the *first* opening tag at
//!    each top-level block boundary. Top-level blocks are
//!    identified positionally — the Nth top-level element in the
//!    HTML corresponds to the Nth child of the comrak root.
//!
//! Why string-level injection rather than custom rendering: comrak's
//! `format_html` doesn't expose a per-node attribute hook on every
//! block kind we care about. A scan-and-inject pass is O(html) and
//! deterministic; the per-element overhead is dwarfed by the comrak
//! formatter's own cost.

use comrak::nodes::AstNode;

/// 1-based source line for each top-level child, in document order.
pub(crate) fn collect_top_level_lines<'a>(root: &'a AstNode<'a>) -> Vec<usize> {
    let mut out = Vec::new();
    for child in root.children() {
        let line = child.data.borrow().sourcepos.start.line;
        // sourcepos is 1-based but defaults to 0 for synthetic nodes;
        // clamp to >=1 so the attribute value is always meaningful.
        out.push(line.max(1));
    }
    out
}

/// Insert `data-afm-source-line="N"` into the first opening tag at
/// each top-level block boundary. Tags considered top-level are
/// `<p>`, `<h1..h6>`, `<ul>`, `<ol>`, `<blockquote>`, `<pre>`,
/// `<table>`, `<hr>`, `<div>` (containers).
pub(crate) fn inject_anchors(html: &str, lines: &[usize]) -> String {
    if lines.is_empty() {
        return html.to_owned();
    }
    let mut out = String::with_capacity(html.len() + lines.len() * 24);
    let mut idx = 0_usize;
    let bytes = html.as_bytes();
    let mut next_line = 0_usize;
    let mut depth: i32 = 0;
    while idx < bytes.len() {
        let b = bytes[idx];
        if b == b'<' && idx + 1 < bytes.len() && bytes[idx + 1] != b'/' {
            // Possible opening tag (we ignore comments / declarations
            // here — comrak doesn't emit them at the top level).
            if let Some(tag_end) = find_tag_end(bytes, idx) {
                let tag_slice = &html[idx..tag_end];
                if depth == 0 && next_line < lines.len() && is_top_level_tag(tag_slice) {
                    out.push_str(&inject_attribute(tag_slice, lines[next_line]));
                    next_line += 1;
                } else {
                    out.push_str(tag_slice);
                }
                if !tag_slice.ends_with("/>") && !is_void_tag(tag_slice) {
                    depth += 1;
                }
                idx = tag_end;
                continue;
            }
        }
        if b == b'<' && idx + 1 < bytes.len() && bytes[idx + 1] == b'/' {
            // Closing tag.
            if let Some(tag_end) = find_tag_end(bytes, idx) {
                out.push_str(&html[idx..tag_end]);
                depth = (depth - 1).max(0);
                idx = tag_end;
                continue;
            }
        }
        out.push(b as char);
        idx += 1;
    }
    out
}

fn find_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    // Walk forward to the next '>' that is not inside an attribute
    // value. We assume comrak's output is well-formed (no `>` inside
    // attribute strings for the tag types we recognise).
    let mut i = start;
    let mut in_quote: Option<u8> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match in_quote {
            None => match c {
                b'"' | b'\'' => in_quote = Some(c),
                b'>' => return Some(i + 1),
                _ => {}
            },
            Some(q) if q == c => in_quote = None,
            _ => {}
        }
        i += 1;
    }
    None
}

fn is_top_level_tag(tag: &str) -> bool {
    let name = tag_name(tag);
    matches!(
        name,
        "p" | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "blockquote"
            | "pre"
            | "table"
            | "hr"
            | "div"
            | "section"
            | "details"
    )
}

fn is_void_tag(tag: &str) -> bool {
    // Only relevant for the depth tracker; comrak emits `<hr>` and
    // `<br>` at top level. Comrak does not currently use the XHTML
    // self-closing form (`<br />`) by default.
    let name = tag_name(tag);
    matches!(name, "hr" | "br" | "img" | "input")
}

fn tag_name(tag: &str) -> &str {
    let body = tag.trim_start_matches('<').trim_end_matches('>');
    let body = body.trim_start_matches('/');
    body.split(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .next()
        .unwrap_or("")
}

fn inject_attribute(tag: &str, line: usize) -> String {
    if !tag.starts_with('<') {
        return tag.to_owned();
    }
    // Insert `data-afm-source-line="N"` immediately after the tag
    // name. We walk to the first whitespace, '/', or '>' to find
    // the insertion point.
    let bytes = tag.as_bytes();
    let mut i = 1; // skip '<'
    while i < bytes.len() {
        let c = bytes[i];
        if c == b' ' || c == b'\t' || c == b'/' || c == b'>' {
            break;
        }
        i += 1;
    }
    let mut out = String::with_capacity(tag.len() + 28);
    out.push_str(&tag[..i]);
    out.push_str(" data-afm-source-line=\"");
    out.push_str(&line.to_string());
    out.push('"');
    out.push_str(&tag[i..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_anchor_into_first_paragraph() {
        let out = inject_anchors("<p>hello</p>", &[1]);
        assert_eq!(out, r#"<p data-afm-source-line="1">hello</p>"#);
    }

    #[test]
    fn injects_anchors_for_multiple_top_level_blocks() {
        let out = inject_anchors("<h1>a</h1><p>b</p>", &[1, 3]);
        assert!(out.contains(r#"<h1 data-afm-source-line="1">"#));
        assert!(out.contains(r#"<p data-afm-source-line="3">"#));
    }

    #[test]
    fn does_not_anchor_nested_blocks() {
        // Only the outer <blockquote> gets the anchor, not the inner <p>.
        let out = inject_anchors("<blockquote><p>x</p></blockquote>", &[1]);
        assert!(out.contains(r#"<blockquote data-afm-source-line="1">"#));
        assert!(!out.contains(r"<p data-afm-source-line="));
    }

    #[test]
    fn no_op_when_lines_is_empty() {
        let html = "<p>x</p>";
        assert_eq!(inject_anchors(html, &[]), html);
    }

    #[test]
    fn handles_void_tags_at_top_level() {
        let out = inject_anchors("<hr><p>x</p>", &[1, 2]);
        assert!(out.contains(r#"<hr data-afm-source-line="1">"#));
        assert!(out.contains(r#"<p data-afm-source-line="2">"#));
    }

    #[test]
    fn ignores_inline_tags() {
        let out = inject_anchors("<p><strong>x</strong></p>", &[1]);
        assert!(out.contains(r#"<p data-afm-source-line="1">"#));
        assert!(!out.contains(r"<strong data-afm-source-line="));
    }

    #[test]
    fn tag_name_extracts_the_lower_case_element_name() {
        assert_eq!(tag_name("<p>"), "p");
        assert_eq!(tag_name("<p class=\"x\">"), "p");
        assert_eq!(tag_name("</p>"), "p");
        assert_eq!(tag_name("<hr/>"), "hr");
    }
}
