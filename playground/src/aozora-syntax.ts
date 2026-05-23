// CodeMirror 6 highlighting overlay for the 青空文庫記法 subset that lives
// inside afm source. Composed on top of `@codemirror/lang-markdown` as a
// ViewPlugin + Decoration set, so Markdown's own highlighting (headings,
// emphasis, code fences) keeps working alongside it.
//
// What we recognise (matches the subset the upstream lexer claims):
//
//   ｜BASE《RUBY》            — explicit-bar ruby
//   KANJI《RUBY》             — implicit-base ruby (one or more CJK chars)
//   ［＃ANNOTATION］          — annotation
//   ※［＃ANNOTATION］        — gaiji annotation (※ marker + bracket pair)
//
// We do NOT try to be a parser. Improperly nested or stray brackets
// render as plain Markdown text; the authoritative parse happens
// server-side via the wasm pipeline and surfaces in the diagnostics
// drawer.

import { syntaxTree } from '@codemirror/language';
import { RangeSetBuilder } from '@codemirror/state';
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view';

// All ranges are added to the same RangeSetBuilder; the builder requires
// ranges to arrive in (from, to) order, so we collect first and sort
// once before building.
interface PendingMark {
  readonly from: number;
  readonly to: number;
  readonly deco: Decoration;
}

const punctDeco = Decoration.mark({ class: 'cm-afm-punct' });
const rubyBaseDeco = Decoration.mark({ class: 'cm-afm-ruby-base' });
const rubyTextDeco = Decoration.mark({ class: 'cm-afm-ruby-text' });
const annotationBodyDeco = Decoration.mark({ class: 'cm-afm-annotation' });
const gaijiMarkerDeco = Decoration.mark({ class: 'cm-afm-gaiji' });

// Pattern 1: explicit-bar ruby — ｜BASE《RUBY》
//   Group 1: bar (｜)
//   Group 2: base
//   Group 3: opening bracket (《)
//   Group 4: ruby text
//   Group 5: closing bracket (》)
const RX_EXPLICIT_RUBY =
  /(｜)([^《\n｜]+?)(《)([^》\n]+?)(》)/gu;

// Pattern 2: implicit-base ruby — one or more CJK chars followed by 《...》.
// Kept loose because the actual lexer rule is heuristic; we under-decorate
// rather than over-decorate.
//   Group 1: base (CJK run)
//   Group 2: opening bracket (《)
//   Group 3: ruby text
//   Group 4: closing bracket (》)
const RX_IMPLICIT_RUBY =
  /([一-鿿㐀-䶿々ヵヶ々]+)(《)([^》\n]+?)(》)/gu;

// Pattern 3: annotation — optionally preceded by ※ for gaiji.
//   Group 1: ※ (gaiji marker, optional)
//   Group 2: opening bracket pair (［＃)
//   Group 3: body
//   Group 4: closing bracket (］)
const RX_ANNOTATION =
  /(※)?(［＃)([^］\n]*?)(］)/gu;

function scanRange(text: string, base: number, pending: PendingMark[]): void {
  // Annotations win against ruby on overlap — the upstream lexer claims
  // ［＃...］ aggressively, and 《》 inside an annotation body should not
  // be re-decorated as ruby. Mark covered spans here.
  const covered: Array<[number, number]> = [];

  for (const m of text.matchAll(RX_ANNOTATION)) {
    if (m.index === undefined) continue;
    // Required capture groups are always defined when the regex matches.
    // `noUncheckedIndexedAccess` widens them to `T | undefined`; assert
    // the ones we know are required (groups 2/3/4) and keep the optional
    // gaiji marker (group 1) narrowable through its undefined check.
    const gaiji = m[1];
    const open = m[2]!;
    const body = m[3]!;
    const close = m[4]!;
    let cursor = base + m.index;
    const start = cursor;
    if (gaiji !== undefined) {
      pending.push({ from: cursor, to: cursor + gaiji.length, deco: gaijiMarkerDeco });
      cursor += gaiji.length;
    }
    pending.push({ from: cursor, to: cursor + open.length, deco: punctDeco });
    cursor += open.length;
    pending.push({ from: cursor, to: cursor + body.length, deco: annotationBodyDeco });
    cursor += body.length;
    pending.push({ from: cursor, to: cursor + close.length, deco: punctDeco });
    cursor += close.length;
    covered.push([start, cursor]);
  }

  const isCovered = (from: number, to: number): boolean =>
    covered.some(([cf, ct]) => from < ct && to > cf);

  for (const m of text.matchAll(RX_EXPLICIT_RUBY)) {
    if (m.index === undefined) continue;
    const bar = m[1]!;
    const baseSpan = m[2]!;
    const open = m[3]!;
    const ruby = m[4]!;
    const close = m[5]!;
    const start = base + m.index;
    const totalEnd = start + m[0].length;
    if (isCovered(start, totalEnd)) continue;
    let cursor = start;
    pending.push({ from: cursor, to: cursor + bar.length, deco: punctDeco });
    cursor += bar.length;
    pending.push({ from: cursor, to: cursor + baseSpan.length, deco: rubyBaseDeco });
    cursor += baseSpan.length;
    pending.push({ from: cursor, to: cursor + open.length, deco: punctDeco });
    cursor += open.length;
    pending.push({ from: cursor, to: cursor + ruby.length, deco: rubyTextDeco });
    cursor += ruby.length;
    pending.push({ from: cursor, to: cursor + close.length, deco: punctDeco });
    covered.push([start, totalEnd]);
  }

  for (const m of text.matchAll(RX_IMPLICIT_RUBY)) {
    if (m.index === undefined) continue;
    const baseSpan = m[1]!;
    const open = m[2]!;
    const ruby = m[3]!;
    const close = m[4]!;
    const start = base + m.index;
    const totalEnd = start + m[0].length;
    if (isCovered(start, totalEnd)) continue;
    let cursor = start;
    pending.push({ from: cursor, to: cursor + baseSpan.length, deco: rubyBaseDeco });
    cursor += baseSpan.length;
    pending.push({ from: cursor, to: cursor + open.length, deco: punctDeco });
    cursor += open.length;
    pending.push({ from: cursor, to: cursor + ruby.length, deco: rubyTextDeco });
    cursor += ruby.length;
    pending.push({ from: cursor, to: cursor + close.length, deco: punctDeco });
  }
}

function buildDecos(view: EditorView): DecorationSet {
  const pending: PendingMark[] = [];
  for (const { from, to } of view.visibleRanges) {
    // Skip code blocks and inline code — the lexer treats them as literal
    // text too, so highlighting `｜foo《bar》` inside ``` would mislead.
    let scanFrom = from;
    syntaxTree(view.state).iterate({
      from,
      to,
      enter: (node) => {
        if (
          node.name === 'FencedCode' ||
          node.name === 'CodeBlock' ||
          node.name === 'InlineCode'
        ) {
          if (node.from > scanFrom) {
            const slice = view.state.doc.sliceString(scanFrom, node.from);
            scanRange(slice, scanFrom, pending);
          }
          scanFrom = Math.max(scanFrom, node.to);
        }
      },
    });
    if (scanFrom < to) {
      const slice = view.state.doc.sliceString(scanFrom, to);
      scanRange(slice, scanFrom, pending);
    }
  }

  pending.sort((a, b) => a.from - b.from || a.to - b.to);
  const builder = new RangeSetBuilder<Decoration>();
  let lastTo = -1;
  for (const p of pending) {
    if (p.from < lastTo) continue; // skip nested overlaps the matcher missed
    builder.add(p.from, p.to, p.deco);
    lastTo = p.to;
  }
  return builder.finish();
}

const aozoraDecorations = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = buildDecos(view);
    }

    update(update: ViewUpdate): void {
      if (update.docChanged || update.viewportChanged || update.selectionSet) {
        this.decorations = buildDecos(update.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

const aozoraTheme = EditorView.baseTheme({
  '.cm-afm-punct': { color: '#a89770', opacity: '0.85' },
  '.cm-afm-ruby-base': { color: '#1f3b73', fontWeight: '600' },
  '.cm-afm-ruby-text': {
    color: '#7a5cad',
    fontSize: '0.85em',
    fontStyle: 'italic',
  },
  '.cm-afm-annotation': {
    color: '#7a7a7a',
    fontStyle: 'italic',
    backgroundColor: '#f1efe6',
  },
  '.cm-afm-gaiji': { color: '#a8523b', fontWeight: '700' },
});

export const aozoraHighlighting = [aozoraDecorations, aozoraTheme];
