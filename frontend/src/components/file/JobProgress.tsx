import { Component, Show } from 'solid-js';
import { jobProgress } from '../../lib/store';
import { formatDuration, formatPercent } from '../../lib/format';
import styles from './JobProgress.module.css';

const JobProgressPanel: Component = () => {
  const job = () => jobProgress();

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
      <Show when={job()}>
        {(j) => (
          <div class={styles.body}>
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
              <Show when={j().estimated_remaining_secs !== undefined}>
                <div class={styles.stat}>
                  <span class={styles.statLabel}>Remaining</span>
                  <span class={styles.statValue}>
                    {formatDuration(j().estimated_remaining_secs!)}
                  </span>
                </div>
              </Show>
            </div>
          </div>
        )}
      </Show>
    </div>
  );
};

export default JobProgressPanel;
