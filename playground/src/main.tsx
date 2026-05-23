// Solid entry point. Mounts <App /> into #app (the boot overlay sits
// inside #app until the first paint replaces it).

/* @refresh reload */
import { render } from 'solid-js/web';

import App from './App';
import './styles/shell.css';

const root = document.getElementById('app');
if (root === null) {
  throw new Error('#app missing from index.html');
}
render(() => <App />, root);
