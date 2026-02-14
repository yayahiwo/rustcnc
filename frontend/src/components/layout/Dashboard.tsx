import { Component } from 'solid-js';
import Sidebar from './Sidebar';
import StatusBar from './StatusBar';
import DRO from '../dro/DRO';
import JogPad from '../jog/JogPad';
import OverridesPanel from '../overrides/OverridesPanel';
import Console from '../console/Console';
import GCodeViewer from '../gcode/GCodeViewer';
import ToolpathViewer from '../viewer3d/ToolpathViewer';
import JobProgressPanel from '../file/JobProgress';
import FileList from '../file/FileList';
import ControlBar from '../controls/ControlBar';
import styles from './Dashboard.module.css';

const Dashboard: Component = () => {
  return (
    <div class={styles.dashboard}>
      <Sidebar />
      <div class={styles.main}>
        <ControlBar />
        <div class={styles.content}>
          <div class={styles.left}>
            <DRO />
            <JogPad />
            <OverridesPanel />
          </div>
          <div class={styles.center}>
            <ToolpathViewer />
            <GCodeViewer />
          </div>
          <div class={styles.right}>
            <JobProgressPanel />
            <FileList />
            <Console />
          </div>
        </div>
        <StatusBar />
      </div>
    </div>
  );
};

export default Dashboard;
