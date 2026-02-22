import { Component, Show } from 'solid-js';
import { machineState, connected, feedRate, spindleSpeed, jobProgress, systemAlert } from '../../lib/store';
import { ws } from '../../lib/ws';
import { authEnabled, authenticated, authUsername, logout } from '../../lib/auth';
import EmergencyStop from './EmergencyStop';
import LayoutSettings from '../layout/LayoutSettings';
import styles from './ControlBar.module.css';

const ControlBar: Component = () => {
  const isRunning = () => {
    const jp = jobProgress();
    return jp?.state === 'Running';
  };

  const isAlarm = () => machineState() === 'Alarm';

  const handleHome = () => ws.sendConsole('$H');
  const handleUnlock = () => ws.sendConsole('$X');
  const handleReset = () => {
    ws.sendRT('soft_reset');
    // $X unlock is handled by the streamer after Welcome
  };
  const handleLogout = () => void logout();

  return (
    <div class={styles.bar}>
      <div class={styles.left}>
        <span class={styles.logo}>RustCNC</span>
        <Show when={isAlarm()}>
          <button class={styles.btn + ' ' + styles.unlock} onClick={handleUnlock}>
            Unlock
          </button>
        </Show>
      </div>

      <div class={styles.center}>
        <Show when={systemAlert()}>
          <span class={styles.systemAlert}>{systemAlert()}</span>
        </Show>
        <Show when={!systemAlert()}>
          <div class={styles.status}>
            <Show when={connected()} fallback={<span class={styles.statusDisconnected}>Disconnected</span>}>
              <span class={styles.statusState} classList={{
                [styles.stateIdle]: machineState() === 'Idle',
                [styles.stateRun]: machineState() === 'Run',
                [styles.stateHold]: machineState() === 'Hold',
                [styles.stateAlarm]: machineState() === 'Alarm',
                [styles.stateHome]: machineState() === 'Home',
                [styles.stateJog]: machineState() === 'Jog',
              }}>{machineState()}</span>
              <Show when={feedRate() > 0}>
                <span class={styles.statusItem}>F{Math.round(feedRate())}</span>
              </Show>
              <Show when={spindleSpeed() > 0}>
                <span class={styles.statusItem}>S{Math.round(spindleSpeed())}</span>
              </Show>
              <Show when={jobProgress()?.state === 'Running' || jobProgress()?.state === 'Paused'}>
                <span class={styles.statusItem}>{Math.round(jobProgress()!.percent_complete)}%</span>
              </Show>
            </Show>
          </div>
        </Show>
      </div>

      <div class={styles.right}>
        <Show when={authEnabled() && authenticated()}>
          <button class={styles.btn} onClick={handleLogout} title={`Logged in as ${authUsername() ?? ''}`}>
            Logout
          </button>
        </Show>
        <LayoutSettings />
        <button
          class={styles.btn + ' ' + styles.home}
          onClick={handleHome}
          disabled={!connected() || isRunning()}
        >
          Home
        </button>
        <button class={styles.btn + ' ' + styles.reset} onClick={handleReset} title="Soft Reset">
          Reset
        </button>
        <EmergencyStop />
      </div>
    </div>
  );
};

export default ControlBar;
