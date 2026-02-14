import { Component } from 'solid-js';
import { ws } from '../../lib/ws';
import styles from './EmergencyStop.module.css';

const EmergencyStop: Component = () => {
  const handleEStop = () => {
    ws.sendRT('soft_reset');
  };

  return (
    <button class={styles.estop} onClick={handleEStop} title="Emergency Stop (Soft Reset)">
      E-STOP
    </button>
  );
};

export default EmergencyStop;
