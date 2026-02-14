import { Component, onMount, onCleanup, createEffect } from 'solid-js';
import { gcodeFile, workPos, machineState } from '../../lib/store';
import { createScene, disposeScene } from './scene';
import { updateToolpath } from './toolpath';
import { updateToolPosition } from './tool';
import styles from './ToolpathViewer.module.css';

const ToolpathViewer: Component = () => {
  let containerRef: HTMLDivElement | undefined;
  let cleanupScene: (() => void) | null = null;

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

  return (
    <div class={'panel ' + styles.viewer}>
      <div class="panel-header">3D View</div>
      <div class={styles.canvas} ref={containerRef} />
    </div>
  );
};

export default ToolpathViewer;
