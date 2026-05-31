// Top-level Solid component. Owns the shared signals:
//   - source     : current editor text
//   - rendered   : { html, diagnostics } from the wasm pipeline
//   - theme      : 'vertical' | 'horizontal' (driven by createTheme)
// and wires the toolbar / editor / preview / diagnostics together.

import { createMemo, createSignal, onMount, type Component } from 'solid-js';
import type { EditorView } from '@codemirror/view';

import DiagnosticsDrawer from './components/DiagnosticsDrawer';
import EditorPane from './components/EditorPane';
import PreviewPane from './components/PreviewPane';
import Toolbar from './components/Toolbar';
import { loadExamples } from './examples';
import { createColorScheme } from './color-scheme';
import { copyShareLink, decodeSourceFromHash } from './share';
import { createTheme } from './theme-toggle';
import { hashSource, renderAfm, type Diagnostic } from './wasm-loader';

const FALLBACK_SOURCE =
  '# afm playground\n\nここに ｜文章《ぶんしょう》 を書いてみてください。\n';

interface Rendered {
  readonly html: string;
  readonly diagnostics: readonly Diagnostic[];
}

const EMPTY_RENDER: Rendered = { html: '', diagnostics: [] };

function pickInitial(examples: ReturnType<typeof loadExamples>): string {
  const fromHash = decodeSourceFromHash(globalThis.location.hash);
  if (fromHash !== null) return fromHash;
  return examples[0]?.source ?? FALLBACK_SOURCE;
}

const App: Component = () => {
  const examples = loadExamples();
  const [source, setSource] = createSignal(pickInitial(examples));
  const [rendered, setRendered] = createSignal<Rendered>(EMPTY_RENDER);
  const [drawerOpen, setDrawerOpen] = createSignal(true);
  const [toast, setToast] = createSignal<{ message: string; ok: boolean } | null>(null);
  const [editorView, setEditorView] = createSignal<EditorView | null>(null);

  const theme = createTheme();
  const colorScheme = createColorScheme();

  // Render gate: hashSource short-circuits identical source runs through
  // the cache instead of re-invoking the wasm pipeline. Cheap to call
  // (xxh3-64) so it stays in the synchronous reactive graph.
  let lastHash: bigint | null = null;
  let pending: ReturnType<typeof setTimeout> | null = null;

  function runRender(text: string): void {
    let h: bigint;
    try {
      h = hashSource(text);
    } catch (err) {
      setRendered({
        html: '',
        diagnostics: [
          {
            level: 'error',
            source: 'internal',
            code: 'playground::hash',
            message: String(err),
          },
        ],
      });
      return;
    }
    if (lastHash === h) return;
    lastHash = h;

    try {
      const result = renderAfm(text);
      setRendered({ html: result.html, diagnostics: result.diagnostics });
      if (result.diagnostics.length > 0) setDrawerOpen(true);
    } catch (err) {
      setRendered({
        html: '',
        diagnostics: [
          {
            level: 'error',
            source: 'internal',
            code: 'playground::render',
            message: String(err),
          },
        ],
      });
      setDrawerOpen(true);
    }
  }

  function scheduleRender(text: string): void {
    if (pending !== null) clearTimeout(pending);
    pending = setTimeout(() => {
      pending = null;
      runRender(text);
    }, 100);
  }

  function loadExample(slug: string): void {
    const ex = examples.find((e) => e.slug === slug);
    if (ex === undefined) return;
    setSource(ex.source);
    if (pending !== null) clearTimeout(pending);
    pending = null;
    lastHash = null;
    runRender(ex.source);
  }

  function flashToast(message: string, ok: boolean): void {
    setToast({ message, ok });
    setTimeout(() => setToast(null), 1800);
  }

  function share(): void {
    void copyShareLink(source()).then(
      () => flashToast('共有リンクをコピーしました', true),
      (err: unknown) => flashToast(`コピーに失敗: ${String(err)}`, false),
    );
  }

  onMount(() => {
    // Solid's `render()` APPENDS to the mount node — it does not replace
    // existing children. The inline `<div id="boot-overlay">` from
    // index.html therefore survives the mount and (because shell.css
    // styles it `position: fixed; inset: 0; z-index: 10`) covers the
    // whole viewport until we explicitly take it down. Removing it here
    // means the user keeps seeing the "afm を読み込み中…" message until
    // the first render lands; once we reach this callback the editor +
    // preview are already in the DOM and the overlay's job is done.
    document.getElementById('boot-overlay')?.remove();
    runRender(source());
  });

  const html = createMemo(() => rendered().html);
  const diagnostics = createMemo(() => rendered().diagnostics);

  return (
    <>
      <Toolbar
        themeMode={theme.mode}
        onThemeChange={(m) => theme.setMode(m)}
        colorSchemePref={colorScheme.pref}
        onCycleColorScheme={() => colorScheme.cyclePref()}
        examples={examples}
        onLoadExample={loadExample}
        onShare={share}
        editorView={editorView}
      />
      <main class="afm-pg-panes">
        <section class="afm-pg-pane afm-pg-pane-editor" aria-label="エディタ">
          <EditorPane
            value={source()}
            onChange={(value) => {
              setSource(value);
              scheduleRender(value);
            }}
            onReady={setEditorView}
          />
        </section>
        <section class="afm-pg-pane afm-pg-pane-preview" aria-label="プレビュー">
          <PreviewPane html={html} />
          <DiagnosticsDrawer
            diagnostics={diagnostics}
            open={drawerOpen}
            onToggle={() => setDrawerOpen((v) => !v)}
          />
        </section>
      </main>
      <footer class="afm-pg-footer">
        <span>
          Powered by{' '}
          <a href="https://github.com/P4suta/afm" target="_blank" rel="noopener">
            afm
          </a>{' '}
          — Aozora Flavored Markdown
        </span>
      </footer>
      {toast() !== null && (
        <div class="afm-pg-toast" data-ok={toast()!.ok ? '1' : '0'}>
          {toast()!.message}
        </div>
      )}
    </>
  );
};

export default App;
