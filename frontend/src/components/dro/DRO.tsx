import { Component, createSignal } from 'solid-js';
import { machinePos, workPos, machineState } from '../../lib/store';
import { formatCoord } from '../../lib/format';
import AxisDisplay from './AxisDisplay';
import styles from './DRO.module.css';

const DRO: Component = () => {
  const [showWork, setShowWork] = createSignal(true);
  const pos = () => showWork() ? workPos() : machinePos();

  const stateColor = () => {
    const s = machineState();
    const map: Record<string, string> = {
      Idle: 'var(--state-idle)',
      Run: 'var(--state-run)',
      Hold: 'var(--state-hold)',
      Alarm: 'var(--state-alarm)',
      Home: 'var(--state-home)',
      Jog: 'var(--state-jog)',
      Check: 'var(--state-check)',
      Door: 'var(--state-door)',
      Sleep: 'var(--state-sleep)',
    };
    return map[s] || 'var(--text-secondary)';
  };

  return (
    <div class="panel">
      <div class="panel-header">
        <span>Position</span>
        <button
          class={styles.coordToggle}
          onClick={() => setShowWork(!showWork())}
          title={showWork() ? 'Showing Work coordinates' : 'Showing Machine coordinates'}
        >
          {showWork() ? 'WORK' : 'MACH'}
        </button>
      </div>
      <div class={styles.state} style={{ color: stateColor() }}>
        {machineState()}
      </div>
      <div class={styles.axes}>
        <AxisDisplay axis="X" value={formatCoord(pos().x)} color="var(--axis-x)" />
        <AxisDisplay axis="Y" value={formatCoord(pos().y)} color="var(--axis-y)" />
        <AxisDisplay axis="Z" value={formatCoord(pos().z)} color="var(--axis-z)" />
      </div>
    </div>
  );
};

export default DRO;
