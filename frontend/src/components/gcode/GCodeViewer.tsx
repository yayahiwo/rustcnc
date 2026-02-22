import { Component, For, Show, createSignal, createEffect, onCleanup } from 'solid-js';
import { gcodeFile, jobProgress, pickedLineNum, setPickedLineNum } from '../../lib/store';
import styles from './GCodeViewer.module.css';

const GCODE_DESC: Record<string, string> = {
  G0: 'Rapid Move', G1: 'Linear Move', G2: 'CW Arc', G3: 'CCW Arc',
  G4: 'Dwell', G10: 'Set Offsets', G17: 'XY Plane', G18: 'XZ Plane',
  G19: 'YZ Plane', G20: 'Inches', G21: 'Millimeters', G28: 'Home',
  G30: 'Home 2nd Position', G38: 'Probe',
  G40: 'Cutter Comp Off', G43: 'Tool Length Offset', G49: 'Cancel TLO',
  G53: 'Machine Coords', G54: 'Work Coord 1', G55: 'Work Coord 2',
  G56: 'Work Coord 3', G57: 'Work Coord 4', G58: 'Work Coord 5',
  G59: 'Work Coord 6', G80: 'Cancel Canned Cycle',
  G90: 'Absolute Mode', G91: 'Incremental Mode',
  G92: 'Set Position', G93: 'Inverse Time Feed', G94: 'Units/Min Feed',
  M0: 'Program Pause', M1: 'Optional Pause', M2: 'Program End',
  M3: 'Spindle On CW', M4: 'Spindle On CCW', M5: 'Spindle Stop',
  M6: 'Tool Change', M7: 'Mist Coolant', M8: 'Flood Coolant',
  M9: 'Coolant Off', M30: 'Program End & Reset',
};

function getLineDesc(text: string): string | null {
  // Check for G/M codes first
  const gm = text.match(/^\s*([GMgm]\d+)/);
  if (gm) return GCODE_DESC[gm[1].toUpperCase()] ?? null;
  // Check for T (tool select) and S (spindle speed) anywhere on line
  const t = text.match(/\b[Tt](\d+)/);
  if (t) return `Tool #${t[1]}`;
  const s = text.match(/\b[Ss](\d+)/);
  if (s) return `Spindle Speed ${s[1]}`;
  return null;
}

const GCodeViewer: Component = () => {
  const lines = () => gcodeFile()?.lines || [];
  const currentLine = () => jobProgress()?.current_line ?? -1;
  const [height, setHeight] = createSignal(250);
  const [pickMode, setPickMode] = createSignal(false);
  const [infoMode, setInfoMode] = createSignal(false);

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
  createEffect(() => {
    currentLine();
    clearTimeout(scrollTimer);
    scrollTimer = setTimeout(scrollToCurrent, 200);
  });
  onCleanup(() => clearTimeout(scrollTimer));

  // Scroll to line picked from 3D viewer
  createEffect(() => {
    const picked = pickedLineNum();
    if (picked !== null && picked > 0 && containerRef) {
      const el = containerRef.querySelector(`[data-line="${picked}"]`);
      if (el) {
        el.scrollIntoView({ block: 'center', behavior: 'smooth' });
      }
    }
  });

  const handleLineClick = (lineNum: number) => {
    if (!pickMode()) return;
    setPickedLineNum(lineNum);
  };

  const handleResizeStart = (e: MouseEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startH = height();
    const onMove = (ev: MouseEvent) => {
      setHeight(Math.max(100, startH + (ev.clientY - startY)));
    };
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  };

  return (
    <div class={'panel ' + styles.viewer} style={{ height: `${height()}px` }}>
      <div class="panel-header">
        <span>G-Code</span>
        <Show when={gcodeFile()}>
          <span class={styles.fileInfo}>{gcodeFile()!.name} : {gcodeFile()!.lines.length} Lines</span>
        </Show>
        <button
          class={styles.pickBtn + (pickMode() ? ' ' + styles.pickBtnActive : '')}
          onClick={() => setPickMode(!pickMode())}
          title={pickMode() ? 'Disable line picker' : 'Pick line to highlight in 3D'}
        >
          <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
            <circle cx="10" cy="10" r="7" />
            <line x1="10" y1="2" x2="10" y2="6" />
            <line x1="10" y1="14" x2="10" y2="18" />
            <line x1="2" y1="10" x2="6" y2="10" />
            <line x1="14" y1="10" x2="18" y2="10" />
          </svg>
        </button>
        <button
          class={styles.pickBtn + (infoMode() ? ' ' + styles.pickBtnActive : '')}
          onClick={() => setInfoMode(!infoMode())}
          title={infoMode() ? 'Hide command info' : 'Show command descriptions'}
        >
          <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
            <circle cx="10" cy="10" r="8" />
            <line x1="10" y1="9" x2="10" y2="14" />
            <circle cx="10" cy="6.5" r="0.5" fill="currentColor" stroke="none" />
          </svg>
        </button>
      </div>
      <div class={styles.code} ref={containerRef}>
        <Show
          when={lines().length > 0}
          fallback={<div class={styles.empty}>No G-code loaded</div>}
        >
          <For each={lines()}>
            {(line) => (
              <div
                class={styles.line + (pickMode() ? ' ' + styles.linePick : '')}
                classList={{
                  [styles.current]: line.line_num === currentLine(),
                  [styles.executed]: line.line_num < currentLine(),
                  [styles.picked]: line.line_num === pickedLineNum(),
                }}
                data-line={line.line_num}
                onClick={() => handleLineClick(line.line_num)}
              >
                <span class={styles.num}>{line.line_num}</span>
                <span class={styles.text}>{line.text}</span>
                <Show when={infoMode() && getLineDesc(line.text)}>
                  <span class={styles.desc}>{getLineDesc(line.text)}</span>
                </Show>
              </div>
            )}
          </For>
        </Show>
      </div>
      <div class={styles.resizeHandle} onMouseDown={handleResizeStart} />
    </div>
  );
};

export default GCodeViewer;
