import { Component, createSignal, For } from 'solid-js';
import { machinePos, workPos, machineState } from '../../lib/store';
import { layout, ALL_AXES } from '../../lib/widgetStore';
import { formatCoord } from '../../lib/format';
import AxisDisplay from './AxisDisplay';
import styles from './DRO.module.css';
import type { Position } from '../../lib/types';

const DRO: Component = () => {
  const [showWork, setShowWork] = createSignal(true);
  const pos = () => showWork() ? workPos() : machinePos();

  const visibleAxes = () => ALL_AXES.slice(0, layout().axisCount);

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
        <For each={visibleAxes()}>
          {(axis) => (
            <AxisDisplay
              axis={axis.name}
              value={formatCoord((pos() as Record<string, number | undefined>)[axis.key] ?? 0)}
              color={axis.color}
            />
          )}
        </For>
      </div>
    </div>
  );
};

export default DRO;
