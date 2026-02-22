import { Component } from 'solid-js';
import { ws } from '../../lib/ws';
import styles from './AxisDisplay.module.css';

interface Props {
  axis: string;
  value: string;
  color: string;
  disabled?: boolean;
}

const AxisDisplay: Component<Props> = (props) => {
  const handleZero = () => {
    if (props.disabled) return;
    const axis = props.axis.toUpperCase();
    ws.sendConsole(`G10 L20 P1 ${axis}0`);
  };

  return (
    <div class={styles.row}>
      <span class={styles.label} style={{ color: props.color }}>
        {props.axis}
      </span>
      <span class={styles.value}>
        {props.value}
      </span>
      <button class={styles.zero} onClick={handleZero} disabled={props.disabled} title={`Zero ${props.axis} axis`}>
        0
      </button>
    </div>
  );
};

export default AxisDisplay;
