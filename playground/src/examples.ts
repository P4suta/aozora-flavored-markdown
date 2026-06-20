// Starter snippet catalogue.
//
// Files are loaded eagerly via Vite's `import.meta.glob('?raw')` so the
// production bundle ships their content inline (no extra fetch on
// dropdown change).

const rawModules = import.meta.glob<string>('../examples/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
});

interface ExampleLabelEntry {
  readonly slug: string;
  readonly label: string;
}

const ORDERED_LABELS: readonly ExampleLabelEntry[] = [
  { slug: '01-welcome', label: 'はじめに ― aozora-md へようこそ' },
  { slug: '02-ruby-furigana', label: 'ルビ (｜ … 《 … 》)' },
  { slug: '03-bouten', label: '傍点 (［＃「…」に傍点］)' },
  { slug: '04-tate-chu-yoko', label: '縦中横' },
  { slug: '05-breaks-and-indent', label: '改ページ・字下げ・段組' },
  { slug: '06-paired-containers', label: '罫囲み・割注などの対構造' },
  { slug: '07-gfm-mixed', label: 'GFM × 青空文庫 (表・タスクリスト)' },
];

export interface Example {
  readonly slug: string;
  readonly label: string;
  readonly source: string;
}

export function loadExamples(): readonly Example[] {
  const bySlug = new Map<string, string>();
  for (const [path, source] of Object.entries(rawModules)) {
    const m = /\/(\d+-[a-z-]+)\.md$/.exec(path);
    if (m && m[1] !== undefined) bySlug.set(m[1], source);
  }
  return ORDERED_LABELS.flatMap(({ slug, label }) => {
    const source = bySlug.get(slug);
    return source !== undefined ? [{ slug, label, source }] : [];
  });
}
