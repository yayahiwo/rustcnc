import { Component } from 'solid-js';
import { ws } from '../../lib/ws';
import styles from './AxisDisplay.module.css';

interface Props {
  axis: string;
  value: string;
  color: string;
}

const AxisDisplay: Component<Props> = (props) => {
  const handleZero = () => {
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
      <button class={styles.zero} onClick={handleZero} title={`Zero ${props.axis} axis`}>
        0
      </button>
    </div>
  );
};

export default AxisDisplay;
