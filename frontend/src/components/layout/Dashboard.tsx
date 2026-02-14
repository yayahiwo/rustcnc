import { Component, For, Index, createSignal, createMemo } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import Sidebar from './Sidebar';
import StatusBar from './StatusBar';
import Widget from './Widget';
import DRO from '../dro/DRO';
import JogPad from '../jog/JogPad';
import OverridesPanel from '../overrides/OverridesPanel';
import Console from '../console/Console';
import GCodeViewer from '../gcode/GCodeViewer';
import ToolpathViewer from '../viewer3d/ToolpathViewer';
import JobProgressPanel from '../file/JobProgress';
import FileList from '../file/FileList';
import ControlBar from '../controls/ControlBar';
import { layout, moveWidget } from '../../lib/widgetStore';
import type { WidgetId } from '../../lib/widgetStore';
import styles from './Dashboard.module.css';

// ── Component lookup ──

const WIDGET_COMPONENTS: Record<WidgetId, Component> = {
  'dro': DRO,
  'jogpad': JogPad,
  'overrides': OverridesPanel,
  'toolpath-viewer': ToolpathViewer,
  'gcode-viewer': GCodeViewer,
  'job-progress': JobProgressPanel,
  'file-list': FileList,
  'console': Console,
};

// ── Drop handling ──

function calcInsertIndex(e: DragEvent, container: HTMLElement): number {
  const children = Array.from(container.children) as HTMLElement[];
  const mouseY = e.clientY;

  for (let i = 0; i < children.length; i++) {
    const rect = children[i].getBoundingClientRect();
    const midY = rect.top + rect.height / 2;
    if (mouseY < midY) return i;
  }
  return children.length;
}

// ── Column component ──

interface ColumnProps {
  columnIndex: number;
}

const Column: Component<ColumnProps> = (props) => {
  const [dragOver, setDragOver] = createSignal(false);
  let colRef: HTMLDivElement | undefined;

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    setDragOver(true);
  };

  const handleDragLeave = (e: DragEvent) => {
    if (colRef && !colRef.contains(e.relatedTarget as Node)) {
      setDragOver(false);
    }
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const widgetId = e.dataTransfer?.getData('text/plain') as WidgetId;
    if (!widgetId || !colRef) return;
    const idx = calcInsertIndex(e, colRef);
    moveWidget(widgetId, props.columnIndex, idx);
  };

  return (
    <div
      ref={colRef}
      class={styles.column + (dragOver() ? ' ' + styles.dropTarget : '')}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      <For each={layout().columns[props.columnIndex]}>
        {(widgetId) => (
          <Widget id={widgetId}>
            <Dynamic component={WIDGET_COMPONENTS[widgetId]} />
          </Widget>
        )}
      </For>
    </div>
  );
};

// ── Dashboard ──

const Dashboard: Component = () => {
  const gridTemplate = () =>
    layout().columnWidths.map((w) => `${w}fr`).join(' ');

  const columnIndices = createMemo(() =>
    Array.from({ length: layout().columnCount }, (_, i) => i)
  );

  return (
    <div class={styles.dashboard}>
      <Sidebar />
      <div class={styles.main}>
        <ControlBar />
        <div
          class={styles.content}
          style={{ 'grid-template-columns': gridTemplate() }}
        >
          <Index each={columnIndices()}>
            {(colIndex) => <Column columnIndex={colIndex()} />}
          </Index>
        </div>
        <StatusBar />
      </div>
    </div>
  );
};

export default Dashboard;
