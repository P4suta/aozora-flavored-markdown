// CodeMirror lives outside Solid's reactivity graph. We mount it once
// in onMount, dispose in onCleanup, and forward `props.value` changes
// imperatively through a createEffect (the inner guard prevents echoing
// the change back through onChange).

import { createEffect, onCleanup, onMount, type Component } from 'solid-js';

import { createEditor, type EditorHandle } from '../editor';

interface EditorPaneProps {
  value: string;
  onChange(value: string): void;
}

const EditorPane: Component<EditorPaneProps> = (props) => {
  let mount: HTMLDivElement | undefined;
  let handle: EditorHandle | undefined;

  onMount(() => {
    if (mount === undefined) return;
    handle = createEditor(mount, props.value, (next) => {
      props.onChange(next);
    });
  });

  createEffect(() => {
    const next = props.value;
    if (handle === undefined) return;
    if (handle.getValue() === next) return;
    handle.setValue(next);
  });

  onCleanup(() => {
    handle?.view.destroy();
    handle = undefined;
  });

  return <div class="afm-pg-editor-mount" ref={mount} />;
};

export default EditorPane;
