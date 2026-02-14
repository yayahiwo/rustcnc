import { Component, Show } from 'solid-js';
import { machineState, feedRate, spindleSpeed, lineNumber, wsConnected, connected } from '../../lib/store';
import { formatFeed, formatRPM } from '../../lib/format';
import styles from './StatusBar.module.css';

const StatusBar: Component = () => {
  return (
    <div class={styles.bar}>
      <div class={styles.left}>
        <span
          class={styles.indicator}
          classList={{ [styles.online]: wsConnected(), [styles.offline]: !wsConnected() }}
        />
        <span class={styles.label}>WS: {wsConnected() ? 'Connected' : 'Disconnected'}</span>
        <span class={styles.sep}>|</span>
        <span class={styles.label}>GRBL: {connected() ? 'Online' : 'Offline'}</span>
      </div>
      <div class={styles.center}>
        <span class={styles.label}>F: {formatFeed(feedRate())} mm/min</span>
        <span class={styles.sep}>|</span>
        <span class={styles.label}>S: {formatRPM(spindleSpeed())} RPM</span>
        <Show when={lineNumber() > 0}>
          <span class={styles.sep}>|</span>
          <span class={styles.label}>N{lineNumber()}</span>
        </Show>
      </div>
      <div class={styles.right}>
        <span class={styles.stateLabel}>
          {machineState()}
        </span>
      </div>
    </div>
  );
};

export default StatusBar;
