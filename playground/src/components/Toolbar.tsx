import { For, type Accessor, type Component } from 'solid-js';
import type { EditorView } from '@codemirror/view';

import type { ColorSchemePref } from '../color-scheme';
import { WRAP_SHAPES, wrapCommand } from '../editor/wrapCommands';
import type { Example } from '../examples';
import type { ThemeMode } from '../styles/theme-urls';

interface ToolbarProps {
  themeMode: Accessor<ThemeMode>;
  onThemeChange(mode: ThemeMode): void;
  colorSchemePref: Accessor<ColorSchemePref>;
  onCycleColorScheme(): void;
  examples: readonly Example[];
  onLoadExample(slug: string): void;
  onShare(): void;
  editorView: Accessor<EditorView | null>;
  onShowGuide(): void;
}

const COLOR_SCHEME_LABEL: Record<ColorSchemePref, string> = {
  auto: '🌗 自動',
  light: '☀ ライト',
  dark: '🌙 ダーク',
};

const Toolbar: Component<ToolbarProps> = (props) => {
  return (
    <header class="afm-pg-toolbar">
      <a
        class="afm-pg-brand"
        href="https://github.com/P4suta/afm"
        target="_blank"
        rel="noopener"
      >
        <span class="afm-pg-brand-mark">afm</span>
        <span class="afm-pg-brand-sub">playground</span>
      </a>
      <div class="afm-pg-toolbar-group" role="group" aria-label="表示モード">
        <span class="afm-pg-label">表示</span>
        <button
          type="button"
          class="afm-pg-toggle"
          aria-pressed={props.themeMode() === 'vertical' ? 'true' : 'false'}
          onClick={() => props.onThemeChange('vertical')}
        >
          縦書き
        </button>
        <button
          type="button"
          class="afm-pg-toggle"
          aria-pressed={props.themeMode() === 'horizontal' ? 'true' : 'false'}
          onClick={() => props.onThemeChange('horizontal')}
        >
          横書き
        </button>
      </div>
      <div class="afm-pg-toolbar-group">
        <label class="afm-pg-label" for="afm-pg-example">
          例文
        </label>
        <select
          id="afm-pg-example"
          class="afm-pg-select"
          onChange={(event) => {
            const target = event.currentTarget;
            const slug = target.value;
            if (slug === '') return;
            props.onLoadExample(slug);
            target.value = '';
          }}
        >
          <option value="">例文を読み込む…</option>
          <For each={props.examples}>
            {(ex) => <option value={ex.slug}>{ex.label}</option>}
          </For>
        </select>
      </div>
      <div class="afm-pg-toolbar-group">
        <label class="afm-pg-label" for="afm-pg-wrap">
          囲む
        </label>
        <select
          id="afm-pg-wrap"
          class="afm-pg-select afm-pg-select-wrap"
          title="選択範囲を青空文庫記法で囲む"
          onChange={(event) => {
            const target = event.currentTarget;
            const id = target.value;
            target.value = '';
            const view = props.editorView();
            if (id === '' || !view) return;
            const shape = WRAP_SHAPES.find((s) => s.id === id);
            if (!shape) return;
            wrapCommand(shape)(view);
            view.focus();
          }}
        >
          <option value="">記法で囲む…</option>
          <For each={WRAP_SHAPES}>
            {(shape) => <option value={shape.id}>{shape.description}</option>}
          </For>
        </select>
      </div>
      <div class="afm-pg-toolbar-spacer" />
      <button
        type="button"
        class="afm-pg-toggle"
        title="記法リファレンスを開く"
        onClick={() => props.onShowGuide()}
      >
        📖 記法
      </button>
      <button
        type="button"
        class="afm-pg-toggle"
        title="配色を切り替え（自動 / ライト / ダーク）"
        aria-label={`配色: ${COLOR_SCHEME_LABEL[props.colorSchemePref()]}`}
        onClick={() => props.onCycleColorScheme()}
      >
        {COLOR_SCHEME_LABEL[props.colorSchemePref()]}
      </button>
      <button type="button" class="afm-pg-share" onClick={() => props.onShare()}>
        共有リンクをコピー
      </button>
    </header>
  );
};

export default Toolbar;
