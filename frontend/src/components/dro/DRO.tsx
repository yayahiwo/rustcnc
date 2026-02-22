import { Component, createSignal, createEffect, For } from 'solid-js';
import { machinePos, workPos, machineState, connected, jobProgress } from '../../lib/store';
import { layout, ALL_AXES } from '../../lib/widgetStore';
import { formatCoord } from '../../lib/format';
import { ws } from '../../lib/ws';
import AxisDisplay from './AxisDisplay';
import styles from './DRO.module.css';
import type { Position } from '../../lib/types';

interface SavedPosition {
  id: number;
  x: number;
  y: number;
  z: number;
}

function loadSavedPositions(): SavedPosition[] {
  try {
    const raw = localStorage.getItem('rustcnc-saved-positions');
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (p: any) =>
        typeof p.id === 'number' &&
        typeof p.x === 'number' &&
        typeof p.y === 'number' &&
        typeof p.z === 'number'
    );
  } catch {
    return [];
  }
}

const DRO: Component = () => {
  const [showWork, setShowWork] = createSignal(true);
  const pos = () => showWork() ? workPos() : machinePos();

  const [savedPositions, setSavedPositions] = createSignal<SavedPosition[]>(loadSavedPositions());

  createEffect(() => {
    localStorage.setItem('rustcnc-saved-positions', JSON.stringify(savedPositions()));
  });

  const canGo = () => {
    const s = machineState();
    return connected() && (s === 'Idle' || s === 'Jog');
  };

  const hasActiveJob = () => {
    const s = jobProgress()?.state;
    return s === 'Running' || s === 'Paused';
  };

  const handleSavePosition = () => {
    const wp = workPos();
    const nextId = Math.max(...savedPositions().map(p => p.id), 0) + 1;
    setSavedPositions([...savedPositions(), { id: nextId, x: wp.x ?? 0, y: wp.y ?? 0, z: wp.z ?? 0 }]);
  };

  const handleGoToPosition = (p: SavedPosition) => {
    if (!canGo() || hasActiveJob()) return;
    ws.sendJog({ x: p.x, y: p.y, z: p.z }, 1000, false);
  };

  const handleDeletePosition = (id: number) => {
    setSavedPositions(savedPositions().filter(p => p.id !== id));
  };

  const handleGoToZero = () => {
    if (!canGo() || hasActiveJob()) return;
    ws.sendJog({ x: 0, y: 0, z: 0 }, 1000, false);
  };

  const visibleAxes = () => ALL_AXES.slice(0, layout().axisCount);

  const handleZeroAll = () => {
    if (hasActiveJob()) return;
    const axes = visibleAxes().map(a => `${a.name.toUpperCase()}0`).join(' ');
    ws.sendConsole(`G10 L20 P1 ${axes}`);
  };

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
              disabled={hasActiveJob()}
            />
          )}
        </For>
        <button class={styles.zeroAll} onClick={handleZeroAll} disabled={hasActiveJob()} title="Zero all axes">
          Zero All
        </button>
      </div>
      <div class={styles.savedPositions}>
        <For each={savedPositions()}>
          {(sp) => (
            <div class={styles.savedRow}>
              <button
                class={styles.goBtn}
                onClick={() => handleGoToPosition(sp)}
                disabled={!canGo() || hasActiveJob()}
                title="Jog to this position"
              >
                Go
              </button>
              <span class={styles.savedCoords}>
                X:{formatCoord(sp.x)} Y:{formatCoord(sp.y)} Z:{formatCoord(sp.z)}
              </span>
              <button
                class={styles.deleteBtn}
                onClick={() => handleDeletePosition(sp.id)}
                title="Delete saved position"
              >
                &times;
              </button>
            </div>
          )}
        </For>
        <button class={styles.addBtn} onClick={handleSavePosition} title="Save current position">
          +
        </button>
        <button
          class={styles.goToZeroBtn}
          onClick={handleGoToZero}
          disabled={!canGo() || hasActiveJob()}
          title="Jog to X0 Y0 Z0"
        >
          Go To Zero
        </button>
      </div>
    </div>
  );
};

export default DRO;
