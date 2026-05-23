import { For, Show, type Accessor, type Component } from 'solid-js';

import type { Diagnostic } from '../wasm-loader';

interface DiagnosticsDrawerProps {
  diagnostics: Accessor<readonly Diagnostic[]>;
  open: Accessor<boolean>;
  onToggle(): void;
}

const DiagnosticsDrawer: Component<DiagnosticsDrawerProps> = (props) => {
  const count = (): number => props.diagnostics().length;

  return (
    <div class="afm-pg-drawer">
      <button
        type="button"
        class="afm-pg-drawer-toggle"
        aria-expanded={props.open() ? 'true' : 'false'}
        onClick={() => props.onToggle()}
      >
        <span>
          診断 (
          <span class="afm-pg-drawer-count" data-empty={count() === 0 ? '1' : '0'}>
            {count()}
          </span>
          )
        </span>
        <span class="afm-pg-drawer-arrow" aria-hidden="true">
          ▾
        </span>
      </button>
      <div class="afm-pg-drawer-body" hidden={!props.open()}>
        <Show
          when={count() > 0}
          fallback={
            <ul class="afm-pg-diagnostics-list">
              <li class="afm-pg-diagnostics-empty">診断メッセージはありません</li>
            </ul>
          }
        >
          <ul class="afm-pg-diagnostics-list">
            <For each={props.diagnostics()}>
              {(d) => (
                <li class="afm-pg-diagnostics-item" data-level={d.level}>
                  <span class="afm-pg-diagnostics-level">{d.level}</span>
                  <code class="afm-pg-diagnostics-code">{d.code}</code>
                  <span class="afm-pg-diagnostics-message">{d.message}</span>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </div>
    </div>
  );
};

export default DiagnosticsDrawer;
