import { Component, Show } from 'solid-js';
import { machineState, connected, jobProgress } from '../../lib/store';
import { ws } from '../../lib/ws';
import EmergencyStop from './EmergencyStop';
import styles from './ControlBar.module.css';

const ControlBar: Component = () => {
  const isRunning = () => {
    const jp = jobProgress();
    return jp?.state === 'Running';
  };

  const isPaused = () => {
    const jp = jobProgress();
    return jp?.state === 'Paused';
  };

  const hasJob = () => {
    const jp = jobProgress();
    return jp && (jp.state === 'Running' || jp.state === 'Paused');
  };

  const isAlarm = () => machineState() === 'Alarm';

  const handleStart = () => ws.sendJobControl('Start');
  const handlePause = () => ws.sendJobControl('Pause');
  const handleResume = () => ws.sendJobControl('Resume');
  const handleStop = () => ws.sendJobControl('Stop');
  const handleHome = () => ws.sendConsole('$H');
  const handleUnlock = () => ws.sendConsole('$X');

  return (
    <div class={styles.bar}>
      <div class={styles.left}>
        <Show when={isAlarm()}>
          <button class={styles.btn + ' ' + styles.unlock} onClick={handleUnlock}>
            Unlock
          </button>
        </Show>
        <button
          class={styles.btn + ' ' + styles.home}
          onClick={handleHome}
          disabled={!connected() || isRunning()}
        >
          Home
        </button>
      </div>

      <div class={styles.center}>
        <Show when={!isRunning() && !isPaused()}>
          <button
            class={styles.btn + ' ' + styles.start}
            onClick={handleStart}
            disabled={!connected()}
          >
            Start
          </button>
        </Show>
        <Show when={isRunning()}>
          <button class={styles.btn + ' ' + styles.pause} onClick={handlePause}>
            Pause
          </button>
        </Show>
        <Show when={isPaused()}>
          <button class={styles.btn + ' ' + styles.resume} onClick={handleResume}>
            Resume
          </button>
        </Show>
        <Show when={hasJob()}>
          <button class={styles.btn + ' ' + styles.stop} onClick={handleStop}>
            Stop
          </button>
        </Show>
      </div>

      <div class={styles.right}>
        <EmergencyStop />
      </div>
    </div>
  );
};

export default ControlBar;
