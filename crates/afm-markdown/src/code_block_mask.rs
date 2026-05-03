//! Pre-/post-process pass that hides 青空文庫 trigger characters
//! inside CommonMark fenced code blocks.
//!
//! ## Why this exists
//!
//! `aozora_pipeline` recognises every `｜` / `《` / `》` / `［` / `］` / `※` /
//! `〔` / `〕` / `「` / `」` as a candidate trigger and rewrites it
//! into a PUA sentinel before comrak ever sees the source. That is
//! exactly what we want for prose; it is exactly what we *don't* want
//! inside a fenced code block, where every byte is supposed to flow
//! through to `<pre><code>` literally.
//!
//! `aozora_pipeline` is intentionally CommonMark-blind (ADR-0010 — the
//! parser core has no opinion on Markdown), so the responsibility for
//! teaching it about code-block context lives here. We:
//!
//! 1. Scan the source line by line and locate every fenced code block
//!    (CommonMark info-string fence: a run of three or more backticks
//!    or three or more tildes after at most three leading spaces,
//!    closed by a same-character run that is at least as long).
//! 2. Replace each Aozora trigger character inside a fence with
//!    [`MASK_CHAR`] (U+E000 — Private Use Area, distinct from the
//!    four sentinels U+E001..U+E004) and stash the original char in
//!    insertion order.
//! 3. After `comrak::format_html`, restore the trigger characters in
//!    the HTML output by walking the originals list in the same
//!    order. `comrak`'s HTML escape only touches `<`, `>`, `&`, `"`;
//!    `MASK_CHAR` flows through untouched.
//!
//! ## Why not `\u{E000}` collisions?
//!
//! `aozora_pipeline`'s Phase 0 already scans for source-supplied PUA
//! characters and emits a `Diagnostic::SourceContainsPua` for any
//! encountered. We pre-scan for `MASK_CHAR` in the *original* source
//! and skip masking entirely if any is present (returning the source
//! verbatim and an empty originals list). That preserves the lexer's
//! diagnostic on the user's pristine input and avoids an
//! ambiguity-of-origin in the unmask step.

use core::cmp::min;

/// Private-use code point used to stand in for an Aozora trigger
/// character that lives inside a fenced code block. Distinct from
/// `aozora_pipeline::INLINE_SENTINEL` (U+E001) and the three block
/// sentinels (U+E002..U+E004), so the masking pass cannot collide
/// with the lexer's own sentinels.
const MASK_CHAR: char = '\u{E000}';

/// Every char `aozora_pipeline` treats as a recogniser trigger. Mirrors
/// the upstream `aozora_pipeline` Phase 1 event tokeniser; if the upstream
/// list grows, this list must follow.
const AOZORA_TRIGGERS: &[char] = &['｜', '《', '》', '［', '］', '※', '〔', '〕', '「', '」'];

/// Mask every Aozora trigger character that appears inside a fenced
/// code block. Returns the modified source and the ordered list of
/// original characters that were replaced (for use by [`unmask_html`]).
///
/// Returns `(source.to_owned(), Vec::new())` and skips masking if the
/// source already contains [`MASK_CHAR`] — see the module docs for
/// the rationale.
#[must_use]
pub(crate) fn mask_code_block_triggers(source: &str) -> (String, Vec<char>) {
    if source.contains(MASK_CHAR) {
        return (source.to_owned(), Vec::new());
    }

    let mut out = String::with_capacity(source.len());
    let mut originals: Vec<char> = Vec::new();
    let mut state = MaskState::Outside;

    for line in source.split_inclusive('\n') {
        match state {
            MaskState::Outside => {
                out.push_str(line);
                if let Some(fence) = parse_fence_open(line) {
                    state = MaskState::Inside(fence);
                }
            }
            MaskState::Inside(open) => {
                if is_fence_close(line, open) {
                    out.push_str(line);
                    state = MaskState::Outside;
                } else {
                    for ch in line.chars() {
                        if AOZORA_TRIGGERS.contains(&ch) {
                            originals.push(ch);
                            out.push(MASK_CHAR);
                        } else {
                            out.push(ch);
                        }
                    }
                }
            }
        }
    }

    (out, originals)
}

/// Reverse the masking. For every [`MASK_CHAR`] in `html`, take the
/// next entry from `originals` (in source-scan order, which matches
/// the order they appear in HTML).
///
/// If `originals` runs short, remaining `MASK_CHAR`s flow through
/// unchanged — that is benign because they would be rendered as a
/// PUA glyph in the browser and never collide with body text.
#[must_use]
pub(crate) fn unmask_html(html: &str, originals: &[char]) -> String {
    if originals.is_empty() {
        return html.to_owned();
    }
    let mut out = String::with_capacity(html.len());
    let mut idx = 0;
    for ch in html.chars() {
        if ch == MASK_CHAR && idx < originals.len() {
            out.push(originals[idx]);
            idx += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
enum MaskState {
    Outside,
    Inside(FenceOpen),
}

#[derive(Debug, Clone, Copy)]
struct FenceOpen {
    /// Backtick or tilde — the fence character chosen on the open line.
    marker: char,
    /// Number of consecutive marker chars in the opening fence.
    width: usize,
}

/// Recognise the opening of a fenced code block on this line.
/// CommonMark allows up to 3 leading spaces before the fence run.
/// Returns the fence shape if `line` is a valid open fence.
fn parse_fence_open(line: &str) -> Option<FenceOpen> {
    let stripped = trim_leading_indent(line, 3);
    let bytes = stripped.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let marker = match bytes[0] {
        b'`' => '`',
        b'~' => '~',
        _ => return None,
    };
    let width = bytes.iter().take_while(|&&b| b == bytes[0]).count();
    if width < 3 {
        return None;
    }
    // CommonMark forbids backticks in the info string of a backtick
    // fence (it would defeat closure detection). We don't need to
    // honour that to detect *opens* — the close-detector below
    // re-checks marker char + width independently.
    Some(FenceOpen { marker, width })
}

/// Recognise a closing fence: same marker char as `open`, at least
/// `open.width` repetitions, optional leading indent up to 3 spaces,
/// nothing but whitespace after the run.
fn is_fence_close(line: &str, open: FenceOpen) -> bool {
    let stripped = trim_leading_indent(line, 3);
    let bytes = stripped.as_bytes();
    let want = match open.marker {
        '`' => b'`',
        '~' => b'~',
        _ => return false,
    };
    let run = bytes.iter().take_while(|&&b| b == want).count();
    if run < open.width {
        return false;
    }
    bytes[run..]
        .iter()
        .all(|&b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

/// Strip up to `max` leading ASCII spaces from `line`. Tabs are not
/// expanded — CommonMark allows them inside the indent budget but
/// our masking pass is a pre-pass for trigger char masking, not a
/// CommonMark conformance check; tabs flow through untouched and the
/// fence-detector simply fails on lines that lead with a tab. That is
/// a strict subset of valid fences but matches every real-world afm
/// source we have seen.
fn trim_leading_indent(line: &str, max: usize) -> &str {
    let bytes = line.as_bytes();
    let cap = min(bytes.len(), max);
    let consumed = bytes.iter().take(cap).take_while(|&&b| b == b' ').count();
    &line[consumed..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_code_block_no_mask() {
        let (out, originals) = mask_code_block_triggers("｜青梅《おうめ》");
        assert_eq!(out, "｜青梅《おうめ》");
        assert!(originals.is_empty());
    }

    #[test]
    fn fenced_code_triggers_get_masked() {
        let src = "before\n```\n｜青梅《おうめ》\n```\nafter";
        let (out, originals) = mask_code_block_triggers(src);
        assert!(!out.contains('｜'), "trigger leaked: {out:?}");
        assert!(!out.contains('《'), "trigger leaked: {out:?}");
        assert!(!out.contains('》'), "trigger leaked: {out:?}");
        // before / after stay untouched
        assert!(out.starts_with("before\n```\n"));
        assert!(out.ends_with("\n```\nafter"));
        assert_eq!(originals, vec!['｜', '《', '》']);
    }

    #[test]
    fn tilde_fence_works_too() {
        let src = "~~~\n［＃改ページ］\n~~~";
        let (out, originals) = mask_code_block_triggers(src);
        assert!(!out.contains('［'));
        assert_eq!(originals, vec!['［', '］']);
    }

    #[test]
    fn close_fence_must_match_marker() {
        // Opened with ``` but closed with ~~~ → still inside the
        // fence; everything to EOF stays masked.
        let src = "```\n｜inside\n~~~\n｜still\n";
        let (_, originals) = mask_code_block_triggers(src);
        assert_eq!(originals, vec!['｜', '｜']);
    }

    #[test]
    fn close_fence_must_be_at_least_as_wide() {
        // Opened with ````, closed with only ``` → not closed.
        let src = "````\n｜inside\n```\n｜still\n";
        let (_, originals) = mask_code_block_triggers(src);
        assert_eq!(originals, vec!['｜', '｜']);
    }

    #[test]
    fn outside_text_is_left_alone() {
        let src = "｜prose《outside》\n```\n｜inside\n```\n｜after《tail》";
        let (out, originals) = mask_code_block_triggers(src);
        assert!(out.contains("｜prose《outside》"), "out: {out}");
        assert!(out.contains("｜after《tail》"), "out: {out}");
        assert_eq!(originals, vec!['｜']);
    }

    #[test]
    fn pre_existing_mask_char_disables_masking() {
        // If the source already contains MASK_CHAR, we cannot
        // distinguish a masked trigger from a literal PUA char on the
        // unmask side, so we bail out and leave aozora-pipeline's own
        // PUA-collision diagnostic in charge.
        let src = "\u{E000}\n```\n｜trigger\n```".to_owned();
        let (out, originals) = mask_code_block_triggers(&src);
        assert_eq!(out, src);
        assert!(originals.is_empty());
    }

    #[test]
    fn unmask_round_trips_fenced_triggers() {
        let src = "```\n｜青梅《おうめ》\n```";
        let (masked, originals) = mask_code_block_triggers(src);
        // Pretend comrak emitted the masked content verbatim inside a
        // <pre><code> block (which is exactly what it does).
        let pseudo_html = format!(
            "<pre><code>{}\n</code></pre>\n",
            &masked[4..masked.len() - 4]
        );
        let restored = unmask_html(&pseudo_html, &originals);
        assert!(restored.contains('｜'), "got: {restored}");
        assert!(restored.contains('《'));
        assert!(restored.contains('》'));
    }

    #[test]
    fn unmask_with_empty_originals_is_a_noop() {
        assert_eq!(unmask_html("hello", &[]), "hello");
    }

    #[test]
    fn unmask_handles_more_mask_chars_than_originals_gracefully() {
        // Edge case: comrak somehow emitted more mask chars than we
        // recorded. The extras flow through verbatim — benign.
        let originals = vec!['｜'];
        let masked = format!("{MASK_CHAR}{MASK_CHAR}");
        let restored = unmask_html(&masked, &originals);
        assert_eq!(restored.chars().filter(|&c| c == '｜').count(), 1);
        assert_eq!(restored.chars().filter(|&c| c == MASK_CHAR).count(), 1);
    }

    #[test]
    fn indent_up_to_three_spaces_does_not_break_fence_detection() {
        let src = "   ```\n｜inside\n   ```\nafter";
        let (_, originals) = mask_code_block_triggers(src);
        assert_eq!(originals, vec!['｜']);
    }

    #[test]
    fn indent_of_four_spaces_disables_the_fence() {
        // Four leading spaces: the line is not a fence open per
        // CommonMark (it would be an indented code block instead, but
        // we don't mask indented code blocks). The trigger remains.
        let src = "    ```\n｜prose\n    ```";
        let (out, originals) = mask_code_block_triggers(src);
        assert!(out.contains('｜'), "out: {out}");
        assert!(originals.is_empty());
    }
}
