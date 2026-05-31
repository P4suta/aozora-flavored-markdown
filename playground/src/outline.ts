// Derive a heading outline from the afm IR.
//
// afm already ships heading positions in its IR (`heading` for Markdown
// `#`, `afmHeading` for 青空文庫 `［＃大見出し］`), each carrying a
// `sourceLine`, so the outline needs no extra WASM call — unlike the
// sibling aozora playground, which reads a dedicated nodes_json. We walk
// the block tree (descending into blockquote / list / container) and
// flatten the headings in document order.

import type { IrBlock, IrDocument, IrInline } from './wasm-loader';

export interface OutlineEntry {
  readonly level: number;
  readonly text: string;
  /** 1-based source line, when the renderer attached one. */
  readonly sourceLine: number | null;
}

/** Flatten an inline run to its visible text (ruby readings excluded). */
function inlineText(nodes: readonly IrInline[]): string {
  let out = '';
  for (const node of nodes) {
    switch (node.kind) {
      case 'text':
      case 'code':
        out += node.value;
        break;
      case 'tcy':
        out += node.text;
        break;
      case 'strong':
      case 'emphasis':
        out += inlineText(node.children);
        break;
      case 'ruby':
      case 'doubleRuby':
        out += inlineText(node.base);
        break;
      case 'bouten':
        out += inlineText(node.children);
        break;
      case 'link':
        out += inlineText(node.children);
        break;
      case 'image':
        out += inlineText(node.alt);
        break;
      case 'gaiji':
        out += node.fallbackText ?? node.description ?? '';
        break;
      // lineBreak / annotation contribute no heading text.
      default:
        break;
    }
  }
  return out;
}

function collect(blocks: readonly IrBlock[], acc: OutlineEntry[]): void {
  for (const block of blocks) {
    switch (block.kind) {
      case 'heading':
        acc.push({
          level: block.level,
          text: inlineText(block.children).trim() || '(無題)',
          sourceLine: block.sourceLine ?? null,
        });
        break;
      case 'blockquote':
      case 'container':
        collect(block.children, acc);
        break;
      case 'list':
        for (const item of block.items) collect(item.children, acc);
        break;
      default:
        break;
    }
  }
}

export function outlineFromIr(ir: IrDocument): OutlineEntry[] {
  const acc: OutlineEntry[] = [];
  collect(ir.blocks, acc);
  return acc;
}
