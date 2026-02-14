import { Component, For, Show, createMemo } from 'solid-js';
import { gcodeFile, jobProgress } from '../../lib/store';
import styles from './GCodeViewer.module.css';

const GCodeViewer: Component = () => {
  const lines = () => gcodeFile()?.lines || [];
  const currentLine = () => jobProgress()?.current_line ?? -1;

  let containerRef: HTMLDivElement | undefined;

  // Scroll to current line during job
  const scrollToCurrent = () => {
    const line = currentLine();
    if (line > 0 && containerRef) {
      const el = containerRef.querySelector(`[data-line="${line}"]`);
      if (el) {
        el.scrollIntoView({ block: 'center', behavior: 'smooth' });
      }
    }
  };

  // Debounce scrolling to avoid hammering during fast updates
  let scrollTimer: ReturnType<typeof setTimeout>;
  createMemo(() => {
    currentLine();
    clearTimeout(scrollTimer);
    scrollTimer = setTimeout(scrollToCurrent, 200);
  });

  return (
    <div class={'panel ' + styles.viewer}>
      <div class="panel-header">
        <span>G-Code</span>
        <Show when={gcodeFile()}>
          <span class={styles.filename}>{gcodeFile()!.name}</span>
        </Show>
      </div>
      <div class={styles.code} ref={containerRef}>
        <Show
          when={lines().length > 0}
          fallback={<div class={styles.empty}>No G-code loaded</div>}
        >
          <For each={lines()}>
            {(line) => (
              <div
                class={styles.line}
                classList={{
                  [styles.current]: line.line_num === currentLine(),
                  [styles.executed]: line.line_num < currentLine(),
                }}
                data-line={line.line_num}
              >
                <span class={styles.num}>{line.line_num}</span>
                <span class={styles.text}>{line.text}</span>
              </div>
            )}
          </For>
        </Show>
      </div>
    </div>
  );
};

export default GCodeViewer;
