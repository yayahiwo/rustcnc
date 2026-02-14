import { Component, JSX, createMemo } from 'solid-js';
import { layout, toggleCollapsed, getWidgetMeta } from '../../lib/widgetStore';
import type { WidgetId } from '../../lib/widgetStore';
import styles from './Widget.module.css';

interface WidgetProps {
  id: WidgetId;
  children: JSX.Element;
}

const Widget: Component<WidgetProps> = (props) => {
  const isCollapsed = createMemo(() => !!layout().collapsed[props.id]);
  const meta = createMemo(() => getWidgetMeta(props.id));

  let dragRef: HTMLDivElement | undefined;

  const handleDragStart = (e: DragEvent) => {
    if (!e.dataTransfer) return;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', props.id);
    // Defer adding the class so the drag image captures the full widget
    requestAnimationFrame(() => {
      dragRef?.classList.add(styles.dragging);
    });
  };

  const handleDragEnd = () => {
    dragRef?.classList.remove(styles.dragging);
  };

  const flexStyle = (): JSX.CSSProperties | undefined => {
    const m = meta();
    if (!m?.flex || isCollapsed()) return undefined;
    return { flex: m.flex, 'min-height': '0' };
  };

  return (
    <div
      ref={dragRef}
      class={styles.widget + (isCollapsed() ? ' ' + styles.collapsed : '')}
      style={flexStyle()}
      draggable={false}
    >
      <div class={styles.controls}>
        <button
          class={styles.controlBtn + ' ' + styles.dragHandle}
          draggable={true}
          onDragStart={handleDragStart}
          onDragEnd={handleDragEnd}
          title="Drag to reorder"
        >
          &#x2630;
        </button>
        <button
          class={styles.controlBtn}
          onClick={() => toggleCollapsed(props.id)}
          title={isCollapsed() ? 'Expand' : 'Collapse'}
        >
          {isCollapsed() ? '\u25B6' : '\u25BC'}
        </button>
      </div>
      {props.children}
    </div>
  );
};

export default Widget;
