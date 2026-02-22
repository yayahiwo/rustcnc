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
          onIncrease={() => ws.sendRT('feed_ovr_plus10')}
          onDecrease={() => ws.sendRT('feed_ovr_minus10')}
          onReset={() => ws.sendRT('feed_ovr_reset')}
        />
        <OverrideSlider
          label="Rapid"
          value={overrides().rapids}
          color="var(--accent-blue)"
          onIncrease={() => ws.sendRT('rapid_ovr_reset')}
          onDecrease={() => ws.sendRT('rapid_ovr_50')}
          onReset={() => ws.sendRT('rapid_ovr_reset')}
        />
        <OverrideSlider
          label="Spindle"
          value={overrides().spindle}
          color="var(--accent-yellow)"
          onIncrease={() => ws.sendRT('spindle_ovr_plus10')}
          onDecrease={() => ws.sendRT('spindle_ovr_minus10')}
          onReset={() => ws.sendRT('spindle_ovr_reset')}
        />
      </div>
    </div>
  );
};

export default OverridesPanel;
