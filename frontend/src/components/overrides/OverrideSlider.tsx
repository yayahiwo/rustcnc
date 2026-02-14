import { Component } from 'solid-js';
import styles from './OverrideSlider.module.css';

interface Props {
  label: string;
  value: number;
  color: string;
  onIncrease: () => void;
  onDecrease: () => void;
  onReset: () => void;
}

const OverrideSlider: Component<Props> = (props) => {
  const barWidth = () => Math.min(Math.max(props.value, 0), 200);

  return (
    <div class={styles.slider}>
      <div class={styles.header}>
        <span class={styles.label}>{props.label}</span>
        <span class={styles.value} style={{ color: props.color }}>
          {props.value}%
        </span>
      </div>
      <div class={styles.track}>
        <div
          class={styles.fill}
          style={{
            width: `${barWidth() / 2}%`,
            background: props.color,
          }}
        />
        <div
          class={styles.marker}
          style={{ left: '50%' }}
        />
      </div>
      <div class={styles.controls}>
        <button class={styles.btn} onClick={props.onDecrease}>-</button>
        <button class={styles.resetBtn} onClick={props.onReset}>100%</button>
        <button class={styles.btn} onClick={props.onIncrease}>+</button>
      </div>
    </div>
  );
};

export default OverrideSlider;
