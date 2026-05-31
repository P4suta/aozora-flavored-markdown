// Editor-feature settings panel. Ported from the sibling aozora
// playground's components/SettingsPanel.tsx, retuned for afm.
//
// Differences from the aozora original:
//   - No theme radio group. afm's light/dark colour scheme lives on the
//     toolbar (color-scheme.ts), so this panel is editor features only.
//   - No "reset storage" action. afm persists source via the share-URL
//     hash, not localStorage, so there is nothing to clear here.
//   - No half→full-width toggle. afm does not port that on-type filter
//     (its triggers — [ * | # — are Markdown syntax in afm).
//   - `view` is an Accessor<EditorView | null> to match the rest of the
//     afm chrome.
//
// The toggles flip CodeMirror Compartments owned by editor.ts; the panel
// only reconfigures them on the live view, so all CodeMirror wiring stays
// in one place.

import { createSignal, onCleanup, Show, type Accessor, type Component } from 'solid-js';
import type { EditorView } from '@codemirror/view';

import { inlayHintsCompartment, structureHighlightCompartment } from '../editor';
import { aozoraDecorations } from '../editor/decorations';
import { aozoraInlayHints } from '../editor/inlayHints';

interface SettingsPanelProps {
  // Live editor view. Null until CodeMirror mounts; toggles applied while
  // null are dropped (the editor starts from the on-mount defaults in
  // editor.ts, which match the initial signal values below).
  view: Accessor<EditorView | null>;
}

const SettingsPanel: Component<SettingsPanelProps> = (props) => {
  const [open, setOpen] = createSignal(false);
  const [inlay, setInlay] = createSignal(true);
  const [structure, setStructure] = createSignal(true);

  let rootEl: HTMLDivElement | undefined;

  function handleClickOutside(event: MouseEvent) {
    if (!open()) return;
    if (rootEl && !rootEl.contains(event.target as Node)) setOpen(false);
  }
  document.addEventListener('mousedown', handleClickOutside);
  onCleanup(() => document.removeEventListener('mousedown', handleClickOutside));

  function toggleInlay() {
    const next = !inlay();
    setInlay(next);
    props.view()?.dispatch({
      effects: inlayHintsCompartment.reconfigure(next ? aozoraInlayHints : []),
    });
  }

  function toggleStructure() {
    const next = !structure();
    setStructure(next);
    props.view()?.dispatch({
      effects: structureHighlightCompartment.reconfigure(next ? aozoraDecorations : []),
    });
  }

  return (
    <div class="afm-pg-settings" ref={rootEl}>
      <button
        type="button"
        class="afm-pg-toggle"
        onClick={() => setOpen((v) => !v)}
        title="エディタ設定"
        aria-haspopup="true"
        aria-expanded={open()}
      >
        ⚙ 設定
      </button>
      <Show when={open()}>
        <div class="afm-pg-settings-popover" role="menu">
          <label class="afm-pg-settings-row">
            <input type="checkbox" checked={structure()} onChange={toggleStructure} />
            <span class="afm-pg-settings-label">
              構造ハイライト
              <span class="afm-pg-settings-sub">見出し・ルビ・傍点・注記を色分け表示</span>
            </span>
          </label>
          <label class="afm-pg-settings-row">
            <input type="checkbox" checked={inlay()} onChange={toggleInlay} />
            <span class="afm-pg-settings-label">
              外字インレイヒント
              <span class="afm-pg-settings-sub">※［＃...］の後ろに →解決字 を表示</span>
            </span>
          </label>
        </div>
      </Show>
    </div>
  );
};

export default SettingsPanel;
