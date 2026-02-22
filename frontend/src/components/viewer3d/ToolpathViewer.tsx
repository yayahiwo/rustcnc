import { Component, onMount, onCleanup, createEffect, createSignal } from 'solid-js';
import { gcodeFile, workPos, machineState, jobProgress, pickedLineNum } from '../../lib/store';
import { createScene, disposeScene, zoomCamera, fitToScene, getControls, setCameraView } from './scene';
import type { ViewPreset } from './scene';
import { updateToolpath, updateToolpathProgress, resetToolpathProgress, pickToolpathLine, highlightToolpathLine } from './toolpath';
import { updateToolPosition } from './tool';
import styles from './ToolpathViewer.module.css';

const HEIGHT_KEY = 'rustcnc-viewer-height';

type CameraMode = 'orbit' | 'pan';

const ToolpathViewer: Component = () => {
  let containerRef: HTMLDivElement | undefined;
  let viewerRef: HTMLDivElement | undefined;
  let cleanupScene: (() => void) | null = null;
  const [mode, setMode] = createSignal<CameraMode>('orbit');
  const [view, setView] = createSignal<ViewPreset>('3d');
  const [tiltLocked, setTiltLocked] = createSignal(false);
  const [pickMode, setPickMode] = createSignal(false);
  let pointerDownPos: { x: number; y: number } | null = null;

  // ── Resize logic ──
  const saved = localStorage.getItem(HEIGHT_KEY);
  const [customHeight, setCustomHeight] = createSignal<number | null>(
    saved ? Number(saved) : null,
  );

  // Apply custom height to parent Widget wrapper
  createEffect(() => {
    const h = customHeight();
    const el = viewerRef?.parentElement; // the Widget wrapper
    if (!el) return;
    if (h !== null) {
      el.style.flex = 'none';
      el.style.height = `${h}px`;
    } else {
      el.style.flex = '';
      el.style.height = '';
    }
  });

  const handleResizeStart = (e: MouseEvent) => {
    e.preventDefault();
    const widgetEl = viewerRef?.parentElement;
    if (!widgetEl) return;
    const startY = e.clientY;
    const startH = widgetEl.getBoundingClientRect().height;

    const onMove = (ev: MouseEvent) => {
      const h = Math.max(150, startH + (ev.clientY - startY));
      setCustomHeight(h);
    };
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      const h = customHeight();
      if (h !== null) {
        localStorage.setItem(HEIGHT_KEY, String(Math.round(h)));
      }
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  };

  const toggleTiltLock = () => {
    const ctrl = getControls();
    if (!ctrl) return;
    const next = !tiltLocked();
    setTiltLocked(next);
    if (next) {
      const polar = ctrl.getPolarAngle();
      ctrl.minPolarAngle = polar;
      ctrl.maxPolarAngle = polar;
    } else {
      ctrl.minPolarAngle = 0;
      ctrl.maxPolarAngle = Math.PI;
    }
  };

  const handlePointerDown = (e: PointerEvent) => {
    if (!pickMode()) return;
    pointerDownPos = { x: e.clientX, y: e.clientY };
  };

  const handleCanvasClick = (e: MouseEvent) => {
    if (!pickMode()) return;
    if (pointerDownPos) {
      const dx = e.clientX - pointerDownPos.x;
      const dy = e.clientY - pointerDownPos.y;
      if (dx * dx + dy * dy > 25) return; // 5px drag threshold
    }
    pickToolpathLine(e);
  };

  const switchMode = (m: CameraMode) => {
    setMode(m);
    const ctrl = getControls();
    if (!ctrl) return;
    if (m === 'pan') {
      ctrl.mouseButtons.LEFT = 2; // PAN
      ctrl.mouseButtons.RIGHT = 0; // ROTATE
    } else {
      ctrl.mouseButtons.LEFT = 0; // ROTATE
      ctrl.mouseButtons.RIGHT = 2; // PAN
    }
  };

  onMount(() => {
    if (!containerRef) return;
    cleanupScene = createScene(containerRef);
  });

  onCleanup(() => {
    if (cleanupScene) cleanupScene();
    disposeScene();
  });

  // Update toolpath when G-code changes
  createEffect(() => {
    const file = gcodeFile();
    if (file) {
      updateToolpath(file);
    }
  });

  // Update tool position at animation frame rate
  createEffect(() => {
    const pos = workPos();
    const state = machineState();
    updateToolPosition(pos.x, pos.y, pos.z, state === 'Run');
  });

  // Update toolpath progress coloring (gray out completed lines)
  createEffect(() => {
    const progress = jobProgress();
    if (!progress) return;
    if (progress.state === 'Running' || progress.state === 'Paused' || progress.state === 'Completed') {
      updateToolpathProgress(progress.current_line);
    } else if (progress.state === 'Idle' || progress.state === 'Cancelled') {
      resetToolpathProgress();
    }
  });

  // Highlight picked line in 3D view (bright yellow)
  createEffect(() => {
    highlightToolpathLine(pickedLineNum());
  });

  return (
    <div class={'panel ' + styles.viewer} ref={viewerRef}>
      <div class="panel-header">3D View</div>
      <div class={styles.canvasWrap}>
        <div
          class={styles.canvas + (pickMode() ? ' ' + styles.pickCursor : '')}
          ref={containerRef}
          onPointerDown={handlePointerDown}
          onClick={handleCanvasClick}
        />
        <div class={styles.resizeHandle} onMouseDown={handleResizeStart} title="Drag to resize height">
          <svg viewBox="0 0 12 6" fill="none" stroke="currentColor" stroke-width="1">
            <line x1="0" y1="1" x2="12" y2="1" />
            <line x1="0" y1="3" x2="12" y2="3" />
            <line x1="0" y1="5" x2="12" y2="5" />
          </svg>
        </div>
        <div class={styles.toolbar}>
          <button
            class={styles.tbBtn}
            onClick={() => zoomCamera(0.3)}
            title="Zoom in"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <circle cx="8.5" cy="8.5" r="5.5" />
              <line x1="12.5" y1="12.5" x2="17" y2="17" />
              <line x1="6" y1="8.5" x2="11" y2="8.5" />
              <line x1="8.5" y1="6" x2="8.5" y2="11" />
            </svg>
          </button>
          <button
            class={styles.tbBtn}
            onClick={() => zoomCamera(-0.3)}
            title="Zoom out"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <circle cx="8.5" cy="8.5" r="5.5" />
              <line x1="12.5" y1="12.5" x2="17" y2="17" />
              <line x1="6" y1="8.5" x2="11" y2="8.5" />
            </svg>
          </button>
          <div class={styles.tbSep} />
          <button
            class={styles.tbBtn + (mode() === 'orbit' ? ' ' + styles.tbBtnActive : '')}
            onClick={() => switchMode('orbit')}
            title="Orbit"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <ellipse cx="10" cy="10" rx="7" ry="3" transform="rotate(-30 10 10)" />
              <circle cx="10" cy="10" r="1.5" fill="currentColor" stroke="none" />
            </svg>
          </button>
          <button
            class={styles.tbBtn + (mode() === 'pan' ? ' ' + styles.tbBtnActive : '')}
            onClick={() => switchMode('pan')}
            title="Pan"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <line x1="10" y1="3" x2="10" y2="17" />
              <line x1="3" y1="10" x2="17" y2="10" />
              <polyline points="7,5.5 10,3 13,5.5" />
              <polyline points="7,14.5 10,17 13,14.5" />
              <polyline points="5.5,7 3,10 5.5,13" />
              <polyline points="14.5,7 17,10 14.5,13" />
            </svg>
          </button>
          <button
            class={styles.tbBtn + (tiltLocked() ? ' ' + styles.tbBtnActive : '')}
            onClick={toggleTiltLock}
            title={tiltLocked() ? 'Unlock tilt' : 'Lock tilt'}
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <rect x="4" y="10" width="12" height="8" rx="1.5" />
              {tiltLocked()
                ? <path d="M7 10V7a3 3 0 0 1 6 0v3" />
                : <path d="M7 10V7a3 3 0 0 1 6 0" />
              }
            </svg>
          </button>
          <button
            class={styles.tbBtn + (pickMode() ? ' ' + styles.tbBtnActive : '')}
            onClick={() => setPickMode(!pickMode())}
            title={pickMode() ? 'Disable line picker' : 'Pick G-code line'}
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <circle cx="10" cy="10" r="7" />
              <line x1="10" y1="2" x2="10" y2="6" />
              <line x1="10" y1="14" x2="10" y2="18" />
              <line x1="2" y1="10" x2="6" y2="10" />
              <line x1="14" y1="10" x2="18" y2="10" />
            </svg>
          </button>
          <div class={styles.tbSep} />
          <button
            class={styles.tbBtn}
            onClick={() => fitToScene()}
            title="Fit to view"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <polyline points="3,7 3,3 7,3" />
              <polyline points="13,3 17,3 17,7" />
              <polyline points="17,13 17,17 13,17" />
              <polyline points="7,17 3,17 3,13" />
              <rect x="6.5" y="6.5" width="7" height="7" rx="1" />
            </svg>
          </button>
          <div class={styles.tbSep} />
          <button
            class={styles.viewBtn + (view() === 'top' ? ' ' + styles.tbBtnActive : '')}
            onClick={() => { setView('top'); setCameraView('top'); }}
            title="Top view (XY)"
          >T</button>
          <button
            class={styles.viewBtn + (view() === 'front' ? ' ' + styles.tbBtnActive : '')}
            onClick={() => { setView('front'); setCameraView('front'); }}
            title="Front view (XZ)"
          >F</button>
          <button
            class={styles.viewBtn + (view() === 'right' ? ' ' + styles.tbBtnActive : '')}
            onClick={() => { setView('right'); setCameraView('right'); }}
            title="Right view (YZ)"
          >R</button>
          <button
            class={styles.viewBtn + (view() === '3d' ? ' ' + styles.tbBtnActive : '')}
            onClick={() => { setView('3d'); setCameraView('3d'); }}
            title="3D perspective"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <path d="M10 3 L17 7 L17 14 L10 18 L3 14 L3 7 Z" />
              <line x1="10" y1="3" x2="10" y2="18" />
              <line x1="3" y1="7" x2="17" y2="7" />
              <line x1="10" y1="10.5" x2="17" y2="14" />
              <line x1="10" y1="10.5" x2="3" y2="14" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
};

export default ToolpathViewer;
