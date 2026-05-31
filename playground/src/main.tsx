// Solid entry point. Mounts <App /> into #app (the boot overlay sits
// inside #app until the first paint replaces it).

/* @refresh reload */
import { render } from 'solid-js/web';

import App from './App';
import { bootstrapColorScheme } from './color-scheme';
import './styles/shell.css';

// Paint the saved light/dark preference before the first render so there
// is no flash of the wrong scheme.
bootstrapColorScheme();

const root = document.getElementById('app');
if (root === null) {
  throw new Error('#app missing from index.html');
}
render(() => <App />, root);
