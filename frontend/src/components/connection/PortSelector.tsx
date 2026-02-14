import { Component, For, Show } from 'solid-js';
import type { PortInfo } from '../../lib/types';
import styles from './PortSelector.module.css';

interface Props {
  ports: PortInfo[];
  selected: string;
  onSelect: (port: string) => void;
  onRefresh: () => void;
}

const PortSelector: Component<Props> = (props) => {
  return (
    <div class={styles.container}>
      <div class={styles.row}>
        <select
          class={styles.select}
          value={props.selected}
          onChange={(e) => props.onSelect(e.currentTarget.value)}
        >
          <option value="">Select port...</option>
          <For each={props.ports}>
            {(port) => (
              <option value={port.path}>
                {port.path}
                {port.manufacturer ? ` (${port.manufacturer})` : ''}
              </option>
            )}
          </For>
        </select>
        <button class={styles.refresh} onClick={props.onRefresh} title="Refresh ports">
          &#x21bb;
        </button>
      </div>
      <Show when={props.ports.length === 0}>
        <span class={styles.hint}>No serial ports found</span>
      </Show>
    </div>
  );
};

export default PortSelector;
