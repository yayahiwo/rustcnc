import { Component } from 'solid-js';
import ConnectionPanel from '../connection/ConnectionPanel';
import styles from './Sidebar.module.css';

const Sidebar: Component = () => {
  return (
    <aside class={styles.sidebar}>
      <div class={styles.logo}>
        <span class={styles.logoText}>RustCNC</span>
        <span class={styles.version}>v0.1.0</span>
      </div>
      <ConnectionPanel />
    </aside>
  );
};

export default Sidebar;
