import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { expect, it } from 'vitest';

it('restores the state selection after an unobserved native selection change', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  const view = new EditorView({
    parent,
    state: EditorState.create({ doc: '｜漢字《》', selection: { anchor: 4 } }),
  });
  try {
    const selection = document.getSelection();
    if (selection === null) throw new Error('The document has no selection');

    view.focus();
    const expected = view.domAtPos(4);
    expect(selection.anchorNode).toBe(expected.node);
    expect(selection.anchorOffset).toBe(expected.offset);

    const start = view.domAtPos(0);
    selection.collapse(start.node, start.offset);
    expect(selection.anchorOffset).toBe(start.offset);
    expect(view.state.selection.main.anchor).toBe(4);

    view.focus();

    expect(selection.anchorNode).toBe(expected.node);
    expect(selection.anchorOffset).toBe(expected.offset);
    expect(view.state.selection.main.anchor).toBe(4);
  } finally {
    view.destroy();
    parent.remove();
  }
});
