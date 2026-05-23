// Vite ?url imports resolve at build time to hashed asset URLs. The two
// theme files live in the mdbook site (single source of truth — no
// copy). Swapping `<link id="afm-theme">.href` between them flips
// the preview between vertical (tategaki) and horizontal layout
// without re-running the wasm pipeline.

import horizontalUrl from '../../../crates/afm-book/theme/afm-horizontal.css?url';
import verticalUrl from '../../../crates/afm-book/theme/afm-vertical.css?url';

export const THEME_URLS = {
  vertical: verticalUrl,
  horizontal: horizontalUrl,
} as const;

export type ThemeMode = keyof typeof THEME_URLS;
