import { Component, createSignal, Show, For } from 'solid-js';
import { connected, machineState } from '../../lib/store';
import { layout, ALL_AXES } from '../../lib/widgetStore';
import { ws } from '../../lib/ws';
import styles from './JogPad.module.css';

const STEP_SIZES = [0.01, 0.1, 1, 10, 100];
const FEED_RATES = [100, 500, 1000, 3000, 5000];

const JogPad: Component = () => {
  const [stepSize, setStepSize] = createSignal(1);
  const [feedRate, setFeedRate] = createSignal(1000);

  const canJog = () => {
    const s = machineState();
    return connected() && (s === 'Idle' || s === 'Jog');
  };

  const jog = (axisKey: string, direction: number) => {
    if (!canJog()) return;
    ws.sendJog({ [axisKey]: direction * stepSize() }, feedRate(), true);
  };

  const jogCancel = () => {
    ws.sendRT('jog_cancel');
  };

  const extraAxes = () => ALL_AXES.slice(3, layout().axisCount);

  return (
    <div class="panel">
      <div class="panel-header">Jog Control</div>
      <div class={styles.body}>
        <div class={styles.grid}>
          {/* Row 1: Y+ and Z+ */}
          <div />
          <button class={styles.jogBtn} onClick={() => jog('y', 1)} disabled={!canJog()}>
            Y+
          </button>
          <div />
          <button class={styles.jogBtn + ' ' + styles.zBtn} onClick={() => jog('z', 1)} disabled={!canJog()}>
            Z+
          </button>

          {/* Row 2: X- and X+ */}
          <button class={styles.jogBtn} onClick={() => jog('x', -1)} disabled={!canJog()}>
            X-
          </button>
          <button class={styles.cancelBtn} onClick={jogCancel} disabled={!canJog()}>
            &#x2716;
          </button>
          <button class={styles.jogBtn} onClick={() => jog('x', 1)} disabled={!canJog()}>
            X+
          </button>
          <div />

          {/* Row 3: Y- and Z- */}
          <div />
          <button class={styles.jogBtn} onClick={() => jog('y', -1)} disabled={!canJog()}>
            Y-
          </button>
          <div />
          <button class={styles.jogBtn + ' ' + styles.zBtn} onClick={() => jog('z', -1)} disabled={!canJog()}>
            Z-
          </button>
        </div>

        {/* Extra axes (A, B, C, U, V) */}
        <Show when={extraAxes().length > 0}>
          <div class={styles.extraAxes}>
            <For each={extraAxes()}>
              {(axis) => (
                <div class={styles.axisRow}>
                  <span class={styles.axisLabel} style={{ color: axis.color }}>
                    {axis.name}
                  </span>
                  <button
                    class={styles.jogBtn}
                    onClick={() => jog(axis.key, -1)}
                    disabled={!canJog()}
                  >
                    {axis.name}-
                  </button>
                  <button
                    class={styles.jogBtn}
                    onClick={() => jog(axis.key, 1)}
                    disabled={!canJog()}
                  >
                    {axis.name}+
                  </button>
                </div>
              )}
            </For>
          </div>
        </Show>

        {/* Step size selector */}
        <div class={styles.presets}>
          <span class={styles.presetLabel}>Step</span>
          <div class={styles.presetRow}>
            {STEP_SIZES.map((size) => (
              <button
                class={styles.presetBtn}
                classList={{ [styles.active]: stepSize() === size }}
                onClick={() => setStepSize(size)}
              >
                {size}
              </button>
            ))}
          </div>
        </div>

        {/* Feed rate selector */}
        <div class={styles.presets}>
          <span class={styles.presetLabel}>Feed</span>
          <div class={styles.presetRow}>
            {FEED_RATES.map((rate) => (
              <button
                class={styles.presetBtn}
                classList={{ [styles.active]: feedRate() === rate }}
                onClick={() => setFeedRate(rate)}
              >
                {rate}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

export default JogPad;
