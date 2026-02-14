import { Component } from 'solid-js';
import { overrides } from '../../lib/store';
import OverrideSlider from './OverrideSlider';
import { ws } from '../../lib/ws';
import styles from './OverridesPanel.module.css';

const OverridesPanel: Component = () => {
  return (
    <div class="panel">
      <div class="panel-header">Overrides</div>
      <div class={styles.body}>
        <OverrideSlider
          label="Feed"
          value={overrides().feed}
          color="var(--accent-green)"
          onIncrease={() => ws.sendRT('feed_override_plus_10')}
          onDecrease={() => ws.sendRT('feed_override_minus_10')}
          onReset={() => ws.sendRT('feed_override_reset')}
        />
        <OverrideSlider
          label="Rapid"
          value={overrides().rapids}
          color="var(--accent-blue)"
          onIncrease={() => ws.sendRT('rapid_override_100')}
          onDecrease={() => ws.sendRT('rapid_override_50')}
          onReset={() => ws.sendRT('rapid_override_100')}
        />
        <OverrideSlider
          label="Spindle"
          value={overrides().spindle}
          color="var(--accent-yellow)"
          onIncrease={() => ws.sendRT('spindle_override_plus_10')}
          onDecrease={() => ws.sendRT('spindle_override_minus_10')}
          onReset={() => ws.sendRT('spindle_override_reset')}
        />
      </div>
    </div>
  );
};

export default OverridesPanel;
