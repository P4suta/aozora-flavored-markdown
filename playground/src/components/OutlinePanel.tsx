// Heading outline. Lists the headings derived from the IR
// (`outline.ts`) and jumps the editor to a heading's source line on
// click. Mirrors the sibling aozora playground's OutlinePanel.

import { For, Show, type Component } from 'solid-js';

import type { OutlineEntry } from '../outline';

interface OutlinePanelProps {
  entries: readonly OutlineEntry[];
  onJump(sourceLine: number): void;
}

const OutlinePanel: Component<OutlinePanelProps> = (props) => {
  return (
    <Show
      when={props.entries.length > 0}
      fallback={<div class="afm-pg-outline-empty">見出しがありません</div>}
    >
      <ul class="afm-pg-outline-list">
        <For each={props.entries}>
          {(h) => (
            <li class={`afm-pg-outline-item afm-pg-outline-l${h.level}`}>
              <button
                type="button"
                class="afm-pg-outline-link"
                disabled={h.sourceLine === null}
                onClick={() => {
                  if (h.sourceLine !== null) props.onJump(h.sourceLine);
                }}
              >
                {h.text}
              </button>
            </li>
          )}
        </For>
      </ul>
    </Show>
  );
};

export default OutlinePanel;
