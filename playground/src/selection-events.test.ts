import { afterEach, beforeEach, describe, expect, it } from 'vitest';

describe('selection event scheduling', () => {
  let selection: Selection;
  let listeners: AbortController;

  const nextTask = (): Promise<void> =>
    new Promise((resolve) => window.setTimeout(resolve));

  beforeEach(async () => {
    const current = document.getSelection();
    if (current === null) throw new Error('The document has no selection');
    selection = current;
    selection.removeAllRanges();
    await nextTask();
    listeners = new AbortController();
  });

  afterEach(async () => {
    listeners.abort();
    selection.removeAllRanges();
    await nextTask();
  });

  it('queues and coalesces changes after the current task', async () => {
    const range = document.createRange();
    const events: {
      targetIsDocument: boolean;
      bubbles: boolean;
      cancelable: boolean;
    }[] = [];
    document.addEventListener(
      'selectionchange',
      (event) => {
        events.push({
          targetIsDocument: event.target === document,
          bubbles: event.bubbles,
          cancelable: event.cancelable,
        });
      },
      { signal: listeners.signal },
    );

    selection.addRange(range);
    selection.removeAllRanges();
    selection.addRange(range);

    expect(selection.getRangeAt(0)).toBe(range);
    expect(events).toEqual([]);
    await Promise.resolve();
    expect(events).toEqual([]);

    await nextTask();
    expect(events).toEqual([
      { targetIsDocument: true, bubbles: false, cancelable: false },
    ]);
  });

  it('schedules another task when a listener changes the selection', async () => {
    const observed: number[] = [];
    const countsAfterMutation: number[] = [];
    const completed = new Promise<void>((resolve) => {
      document.addEventListener(
        'selectionchange',
        () => {
          observed.push(selection.rangeCount);
          if (selection.rangeCount !== 0) {
            selection.removeAllRanges();
            countsAfterMutation.push(observed.length);
          } else {
            resolve();
          }
        },
        { signal: listeners.signal },
      );
    });

    selection.addRange(document.createRange());
    await completed;

    expect(observed).toEqual([1, 0]);
    expect(countsAfterMutation).toEqual([1]);
  });

  it('emits no event when the selection stays unchanged', async () => {
    let eventCount = 0;
    document.addEventListener(
      'selectionchange',
      () => {
        eventCount += 1;
      },
      { signal: listeners.signal },
    );

    selection.removeAllRanges();
    await nextTask();
    expect(eventCount).toBe(0);

    const range = document.createRange();
    selection.addRange(range);
    await nextTask();
    expect(eventCount).toBe(1);

    selection.addRange(range);
    await nextTask();
    expect(eventCount).toBe(1);
  });
});
