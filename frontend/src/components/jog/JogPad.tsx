import { Component, createSignal } from 'solid-js';
import { connected, machineState } from '../../lib/store';
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

  const jog = (x?: number, y?: number, z?: number) => {
    if (!canJog()) return;
    const step = stepSize();
    ws.sendJog(
      x !== undefined ? x * step : undefined,
      y !== undefined ? y * step : undefined,
      z !== undefined ? z * step : undefined,
      feedRate(),
      true,
    );
  };

  const jogCancel = () => {
    ws.sendRT('jog_cancel');
  };

  return (
    <div class="panel">
      <div class="panel-header">Jog Control</div>
      <div class={styles.body}>
        <div class={styles.grid}>
          {/* Row 1: Y+ and Z+ */}
          <div />
          <button class={styles.jogBtn} onClick={() => jog(0, 1)} disabled={!canJog()}>
            Y+
          </button>
          <div />
          <button class={styles.jogBtn + ' ' + styles.zBtn} onClick={() => jog(0, 0, 1)} disabled={!canJog()}>
            Z+
          </button>

          {/* Row 2: X- and X+ */}
          <button class={styles.jogBtn} onClick={() => jog(-1)} disabled={!canJog()}>
            X-
          </button>
          <button class={styles.cancelBtn} onClick={jogCancel} disabled={!canJog()}>
            &#x2716;
          </button>
          <button class={styles.jogBtn} onClick={() => jog(1)} disabled={!canJog()}>
            X+
          </button>
          <div />

          {/* Row 3: Y- and Z- */}
          <div />
          <button class={styles.jogBtn} onClick={() => jog(0, -1)} disabled={!canJog()}>
            Y-
          </button>
          <div />
          <button class={styles.jogBtn + ' ' + styles.zBtn} onClick={() => jog(0, 0, -1)} disabled={!canJog()}>
            Z-
          </button>
        </div>

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
