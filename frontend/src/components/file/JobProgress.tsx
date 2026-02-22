import { Component, For, Show, createSignal, createEffect, on, onCleanup } from 'solid-js';
import { jobProgress, connected, gcodeFile, files, addConsoleLine } from '../../lib/store';
import { ws } from '../../lib/ws';
import { api } from '../../lib/api';
import { formatDuration, formatPercent, formatFileSize } from '../../lib/format';
import type { PauseCondition } from '../../lib/types';
import FileUpload from './FileUpload';
import styles from './JobProgress.module.css';

const JobProgressPanel: Component = () => {
  const job = () => jobProgress();

  const isRunning = () => job()?.state === 'Running';
  const isPaused = () => job()?.state === 'Paused';
  const hasActiveJob = () => {
    const s = job()?.state;
    return s === 'Running' || s === 'Paused';
  };

  // Scheduled pause state
  // Line range inputs
  const [startLineInput, setStartLineInput] = createSignal('');
  const [stopLineInput, setStopLineInput] = createSignal('');

  // Scheduled pause state
  const [scheduledPause, setScheduledPause] = createSignal<PauseCondition | null>(null);
  const [showPauseMenu, setShowPauseMenu] = createSignal(false);
  const [zDepthInput, setZDepthInput] = createSignal('');

  // Countdown timer: ticks down locally between server updates
  const [countdown, setCountdown] = createSignal<number | null>(null);

  // Sync countdown from server estimate
  createEffect(() => {
    const j = job();
    const remaining = j?.estimated_remaining_secs;
    const state = j?.state;
    if (state === 'Running' || state === 'Paused') {
      if (remaining != null && remaining >= 0) {
        setCountdown(remaining);
      }
    } else {
      setCountdown(null);
    }
  });

  // Tick down every second while job is active
  const countdownTimer = setInterval(() => {
    const c = countdown();
    if (c != null && c > 0 && hasActiveJob()) {
      setCountdown(c - 1);
    }
  }, 1000);
  onCleanup(() => clearInterval(countdownTimer));

  // Clear scheduled pause when job stops running
  createEffect(on(() => job()?.state, (state) => {
    if (state !== 'Running') {
      setScheduledPause(null);
      setShowPauseMenu(false);
    }
  }));

  const schedulePause = (condition: PauseCondition) => {
    setScheduledPause(condition);
    setShowPauseMenu(false);
    ws.sendSchedulePause(condition);
  };

  const cancelScheduledPause = () => {
    setScheduledPause(null);
    ws.sendSchedulePause(null);
  };

  const handleSetZDepth = () => {
    const z = parseFloat(zDepthInput());
    if (!isNaN(z)) {
      schedulePause({ AtZDepth: { z } });
      setZDepthInput('');
    }
  };

  const pauseLabel = (): string => {
    const p = scheduledPause();
    if (!p) return '';
    if (p === 'EndOfLayer') return 'End of Layer';
    if (typeof p === 'object' && 'AtZDepth' in p) return `At Z ${p.AtZDepth.z}`;
    return '';
  };

  const stateColor = () => {
    const s = job()?.state;
    switch (s) {
      case 'Running': return 'var(--accent-blue)';
      case 'Paused': return 'var(--accent-yellow)';
      case 'Completed': return 'var(--accent-green)';
      case 'Error': return 'var(--accent-red)';
      case 'Cancelled': return 'var(--accent-orange)';
      default: return 'var(--text-muted)';
    }
  };

  const handleLoad = async (id: string) => {
    try {
      await api.loadFile(id);
    } catch (e) {
      addConsoleLine({ direction: 'System', text: `Failed to load file: ${e}`, timestamp: Date.now() });
    }
  };

  const handleRun = () => {
    const sl = parseInt(startLineInput(), 10);
    const el = parseInt(stopLineInput(), 10);
    const opts: { startLine?: number; stopLine?: number } = {};
    if (!isNaN(sl) && sl > 0) opts.startLine = sl;
    if (!isNaN(el) && el > 0) opts.stopLine = el;
    ws.sendJobControl('Start', opts);
    setStartLineInput('');
    setStopLineInput('');
  };

  const handleDelete = async (id: string) => {
    try {
      await api.deleteFile(id);
    } catch (e) {
      addConsoleLine({ direction: 'System', text: `Failed to delete file: ${e}`, timestamp: Date.now() });
    }
  };

  return (
    <div class="panel">
      <div class="panel-header">
        <span>Job</span>
        <Show when={job()}>
          {(j) => (
            <span class={styles.state} style={{ color: stateColor() }}>
              {j().state}
            </span>
          )}
        </Show>
      </div>
      <div class={styles.body}>
        {/* ── Files section ── */}
        <FileUpload />
        <Show
          when={files().length > 0}
          fallback={<div class={styles.empty}>No files uploaded</div>}
        >
          <div class={styles.fileList}>
            <For each={files()}>
              {(file) => (
                <div class={styles.file}>
                  <div class={styles.fileInfo}>
                    <span class={styles.fileName} title={file.name}>{file.name}</span>
                    <span class={styles.fileMeta}>
                      {formatFileSize(file.size_bytes)} &middot; {file.line_count} lines
                    </span>
                  </div>
                  <div class={styles.fileActions}>
                    <button
                      class={styles.loadBtn}
                      onClick={() => handleLoad(file.id)}
                      title="Load file"
                    >
                      Load
                    </button>
                    <button
                      class={styles.deleteBtn}
                      onClick={() => handleDelete(file.id)}
                      title="Delete file"
                    >
                      &#x2715;
                    </button>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>

        {/* ── Job progress section ── */}
        <Show when={job()}>
          {(j) => (
            <div class={styles.progressSection}>
              <div class={styles.filename}>{j().file_name}</div>
              <div class={styles.progress}>
                <div class={styles.bar}>
                  <div
                    class={styles.fill}
                    style={{ width: `${j().percent_complete}%` }}
                  />
                </div>
                <span class={styles.percent}>{formatPercent(j().percent_complete)}</span>
              </div>
              <Show when={countdown() != null && countdown()! > 0}>
                <div class={styles.countdown}>
                  {formatDuration(countdown()!)} remaining
                </div>
              </Show>
              <div class={styles.stats}>
                <div class={styles.stat}>
                  <span class={styles.statLabel}>Lines</span>
                  <span class={styles.statValue}>
                    {j().current_line} / {j().total_lines}
                  </span>
                </div>
                <div class={styles.stat}>
                  <span class={styles.statLabel}>Elapsed</span>
                  <span class={styles.statValue}>{formatDuration(j().elapsed_secs)}</span>
                </div>
                <Show when={countdown() != null}>
                  <div class={styles.stat}>
                    <span class={styles.statLabel}>Est. Total</span>
                    <span class={styles.statValue}>
                      {formatDuration(j().elapsed_secs + (countdown() ?? 0))}
                    </span>
                  </div>
                </Show>
              </div>
            </div>
          )}
        </Show>

        {/* ── Line range inputs ── */}
        <Show when={!hasActiveJob() && gcodeFile()}>
          <div class={styles.lineRange}>
            <label class={styles.lineLabel}>
              Start Line
              <input
                type="number"
                class={styles.lineInput}
                placeholder="1"
                min="1"
                value={startLineInput()}
                onInput={(e) => setStartLineInput(e.currentTarget.value)}
              />
            </label>
            <label class={styles.lineLabel}>
              Stop Line
              <input
                type="number"
                class={styles.lineInput}
                placeholder="end"
                min="1"
                value={stopLineInput()}
                onInput={(e) => setStopLineInput(e.currentTarget.value)}
              />
            </label>
          </div>
        </Show>

        {/* ── Run controls ── */}
        <div class={styles.actions}>
          {/* Pause / Resume */}
          <button
            class={styles.actionBtn + ' ' + (isPaused() ? styles.resume : styles.pause)}
            onClick={() => isPaused() ? ws.sendJobControl('Resume') : ws.sendJobControl('Pause')}
            disabled={!hasActiveJob()}
            title={isPaused() ? 'Resume' : 'Pause'}
          >
            <Show when={isPaused()} fallback={
              <svg viewBox="0 0 20 20" fill="currentColor" class={styles.btnIcon}>
                <rect x="4" y="3" width="4" height="14" rx="1" />
                <rect x="12" y="3" width="4" height="14" rx="1" />
              </svg>
            }>
              <svg viewBox="0 0 20 20" fill="currentColor" class={styles.btnIcon}>
                <polygon points="5,3 17,10 5,17" />
              </svg>
            </Show>
            {isPaused() ? 'Resume' : 'Pause'}
          </button>
          {/* Cancel */}
          <button
            class={styles.actionBtn + ' ' + styles.cancel}
            onClick={() => ws.sendJobControl('Stop')}
            disabled={!hasActiveJob()}
            title="Cancel"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2.5" class={styles.btnIcon}>
              <line x1="5" y1="5" x2="15" y2="15" />
              <line x1="15" y1="5" x2="5" y2="15" />
            </svg>
            Cancel
          </button>
          {/* Run */}
          <button
            class={styles.actionBtn + ' ' + styles.run}
            onClick={handleRun}
            disabled={!connected() || !gcodeFile() || hasActiveJob()}
            title="Run"
          >
            <svg viewBox="0 0 20 20" fill="currentColor" class={styles.btnIcon}>
              <polygon points="5,3 17,10 5,17" />
            </svg>
            Run
          </button>
        </div>

        {/* ── Scheduled pause ── */}
        <Show when={isRunning()}>
          <Show
            when={scheduledPause()}
            fallback={
              <div class={styles.schedulePauseWrap}>
                <button
                  class={styles.schedulePauseBtn}
                  onClick={() => setShowPauseMenu(!showPauseMenu())}
                >
                  Schedule Pause
                </button>
                <Show when={showPauseMenu()}>
                  <div class={styles.pauseMenu}>
                    <button
                      class={styles.pauseMenuItem}
                      onClick={() => schedulePause('EndOfLayer')}
                    >
                      End of Layer
                    </button>
                    <div class={styles.zDepthRow}>
                      <input
                        type="text"
                        class={styles.zInput}
                        placeholder="Z"
                        value={zDepthInput()}
                        onInput={(e) => setZDepthInput(e.currentTarget.value)}
                        onKeyDown={(e) => { if (e.key === 'Enter') handleSetZDepth(); }}
                      />
                      <button
                        class={styles.pauseMenuItem}
                        onClick={handleSetZDepth}
                      >
                        At Z Depth
                      </button>
                    </div>
                  </div>
                </Show>
              </div>
            }
          >
            <div class={styles.pauseIndicator}>
              <span class={styles.pauseIndicatorLabel}>Pause: {pauseLabel()}</span>
              <button
                class={styles.cancelScheduleBtn}
                onClick={cancelScheduledPause}
              >
                Cancel
              </button>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
};

export default JobProgressPanel;
