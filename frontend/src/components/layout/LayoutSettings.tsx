import { Component, For, Show, createSignal, createEffect, onCleanup } from 'solid-js';
import { layout, setColumnCount, setColumnWidth, setAxisCount } from '../../lib/widgetStore';
import styles from './LayoutSettings.module.css';

const LayoutSettings: Component = () => {
  const [open, setOpen] = createSignal(false);
  let wrapperRef: HTMLDivElement | undefined;

  // Close popover on outside click
  const handleClickOutside = (e: MouseEvent) => {
    if (wrapperRef && !wrapperRef.contains(e.target as Node)) {
      setOpen(false);
    }
  };

  // Only register listener when popover is open
  createEffect(() => {
    if (open()) {
      document.addEventListener('mousedown', handleClickOutside);
      onCleanup(() => document.removeEventListener('mousedown', handleClickOutside));
    }
  });

  return (
    <div class={styles.wrapper} ref={wrapperRef}>
      <button
        class={styles.toggle + (open() ? ' ' + styles.toggleActive : '')}
        onClick={() => setOpen(!open())}
        title="Layout settings"
      >
        &#x2637; Layout
      </button>

      <Show when={open()}>
        <div class={styles.popover}>
          {/* Column count */}
          <div class={styles.section}>
            <div class={styles.label}>Columns</div>
            <div class={styles.countRow}>
              <For each={[1, 2, 3, 4]}>
                {(n) => (
                  <button
                    class={
                      styles.countBtn +
                      (layout().columnCount === n ? ' ' + styles.countBtnActive : '')
                    }
                    onClick={() => setColumnCount(n)}
                  >
                    {n}
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* Axis count */}
          <div class={styles.section}>
            <div class={styles.label}>Axes</div>
            <div class={styles.countRow}>
              <For each={[3, 4, 5, 6, 7, 8]}>
                {(n) => (
                  <button
                    class={
                      styles.countBtn +
                      (layout().axisCount === n ? ' ' + styles.countBtnActive : '')
                    }
                    onClick={() => setAxisCount(n)}
                  >
                    {n}
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* Column widths */}
          <div class={styles.section}>
            <div class={styles.label}>Widths (12-unit grid)</div>
            <For each={layout().columnWidths}>
              {(width, i) => (
                <div class={styles.widthRow}>
                  <span class={styles.widthLabel}>{i() + 1}</span>
                  <div class={styles.widthControls}>
                    <button
                      class={styles.widthBtn}
                      onClick={() => setColumnWidth(i(), width - 1)}
                      disabled={width <= 1}
                    >
                      &minus;
                    </button>
                    <div class={styles.widthBar}>
                      <div
                        class={styles.widthFill}
                        style={{ width: `${(width / 12) * 100}%` }}
                      />
                    </div>
                    <button
                      class={styles.widthBtn}
                      onClick={() => setColumnWidth(i(), width + 1)}
                      disabled={width >= 11}
                    >
                      +
                    </button>
                    <span class={styles.widthValue}>{width}</span>
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default LayoutSettings;
