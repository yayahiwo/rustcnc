import { Component, Show, For } from 'solid-js';
import { systemInfo, connected, wsLatency } from '../../lib/store';
import styles from './SystemInfoPanel.module.css';

function formatUptime(secs: number): string {
  const days = Math.floor(secs / 86400);
  const hours = Math.floor((secs % 86400) / 3600);
  const mins = Math.floor((secs % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${mins}m`;
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m`;
}

function barClass(pct: number): string {
  if (pct > 90) return styles.barDanger;
  if (pct > 70) return styles.barWarn;
  return styles.barNormal;
}

function tempClass(c: number): string {
  if (c > 75) return styles.tempHot;
  if (c > 60) return styles.tempWarm;
  return styles.tempNormal;
}

/** Derive connection type from the serial port path */
function connectionType(port: string): string {
  if (port.includes('ttyACM')) return 'USB CDC';
  if (port.includes('ttyUSB')) return 'USB Serial';
  if (port.includes('ttyAMA') || port.includes('ttyS')) return 'UART';
  if (port.includes('ttyBT') || port.includes('rfcomm')) return 'Bluetooth';
  return 'Serial';
}

/** Display labels for grbl_info keys */
const GRBL_INFO_LABELS: Record<string, string> = {
  'BOARD': 'Board',
  'FIRMWARE': 'Firmware',
  'VER': 'Version',
  'DRIVER': 'Driver',
  'DRIVER VERSION': 'Driver Ver',
  'DRIVER OPTIONS': 'Driver Opts',
  'NVS STORAGE': 'NVS Storage',
  'OPT': 'Options',
  'NEWOPT': 'New Options',
  'AUX INPUTS': 'Aux Inputs',
  'AUX OUTPUTS': 'Aux Outputs',
};

/** Order for displaying grbl_info keys */
const GRBL_INFO_ORDER = [
  'BOARD', 'FIRMWARE', 'VER', 'DRIVER', 'DRIVER VERSION',
  'DRIVER OPTIONS', 'NVS STORAGE', 'OPT', 'NEWOPT',
  'AUX INPUTS', 'AUX OUTPUTS',
];

const SystemInfoPanel: Component = () => {
  const info = () => systemInfo();

  const grblEntries = () => {
    const gi = info()?.grbl_info;
    if (!gi) return [];
    // Show in defined order, then any extras
    const entries: [string, string][] = [];
    const shown = new Set<string>();
    for (const key of GRBL_INFO_ORDER) {
      if (gi[key]) {
        entries.push([GRBL_INFO_LABELS[key] || key, gi[key]]);
        shown.add(key);
      }
    }
    for (const [key, val] of Object.entries(gi)) {
      if (!shown.has(key)) {
        entries.push([GRBL_INFO_LABELS[key] || key, val]);
      }
    }
    return entries;
  };

  const memPct = () => {
    const i = info();
    if (!i || i.memory_total_mb === 0) return 0;
    return Math.round((i.memory_used_mb / i.memory_total_mb) * 100);
  };

  const diskPct = () => {
    const i = info();
    if (!i || i.disk_total_gb === 0) return 0;
    return Math.round((i.disk_used_gb / i.disk_total_gb) * 100);
  };

  return (
    <div class="panel">
      <div class="panel-header">System Info</div>
      <div class={styles.body}>
        {/* Controller Section */}
        <div class={styles.section}>
          <span class={styles.sectionTitle}>Controller</span>
          <Show
            when={grblEntries().length > 0}
            fallback={
              <div class={styles.row}>
                <span class={styles.label}>Firmware</span>
                <span class={styles.value}>
                  {info()?.firmware_version || 'Waiting...'}
                </span>
              </div>
            }
          >
            <For each={grblEntries()}>
              {([label, value]) => (
                <div class={styles.row}>
                  <span class={styles.label}>{label}</span>
                  <span class={styles.value}>{value}</span>
                </div>
              )}
            </For>
          </Show>
        </div>

        <div class={styles.divider} />

        {/* Connection Section */}
        <div class={styles.section}>
          <span class={styles.sectionTitle}>Connection</span>
          <div class={styles.row}>
            <span class={styles.label}>Type</span>
            <span class={styles.value}>
              {info()?.serial_port ? connectionType(info()!.serial_port!) : '—'}
            </span>
          </div>
          <div class={styles.row}>
            <span class={styles.label}>Port</span>
            <span class={styles.value}>
              {info()?.serial_port || '—'}
            </span>
          </div>
          <div class={styles.row}>
            <span class={styles.label}>Status</span>
            <span class={styles.value} style={{ color: connected() ? 'var(--accent-green, #4caf50)' : 'var(--accent-red)' }}>
              {connected() ? 'Connected' : 'Disconnected'}
            </span>
          </div>
          <Show when={connected() && info()}>
            <div class={styles.row}>
              <span class={styles.label}>Connected</span>
              <span class={styles.value}>{formatUptime(info()!.connection_uptime_secs)}</span>
            </div>
          </Show>
        </div>

        <div class={styles.divider} />

        {/* Pi Stats Section */}
        <Show when={info()} fallback={<div class={styles.waiting}>Waiting for system data...</div>}>
          <div class={styles.section}>
            <span class={styles.sectionTitle}>Host System</span>
            {/* CPU Load */}
            <div class={styles.row}>
              <span class={styles.label}>CPU Load</span>
              <span class={styles.value}>
                {info()!.cpu_load[0].toFixed(2)} / {info()!.cpu_load[1].toFixed(2)} / {info()!.cpu_load[2].toFixed(2)}
              </span>
            </div>
            {/* Memory */}
            <div class={styles.row}>
              <span class={styles.label}>RAM</span>
              <div class={styles.valueGroup}>
                <span class={styles.value}>
                  {info()!.memory_used_mb} / {info()!.memory_total_mb} MB
                </span>
                <div class={styles.bar}>
                  <div
                    class={styles.barFill + ' ' + barClass(memPct())}
                    style={{ width: `${memPct()}%` }}
                  />
                </div>
              </div>
            </div>
            {/* Disk */}
            <div class={styles.row}>
              <span class={styles.label}>Disk</span>
              <div class={styles.valueGroup}>
                <span class={styles.value}>
                  {info()!.disk_used_gb.toFixed(1)} / {info()!.disk_total_gb.toFixed(1)} GB
                </span>
                <div class={styles.bar}>
                  <div
                    class={styles.barFill + ' ' + barClass(diskPct())}
                    style={{ width: `${diskPct()}%` }}
                  />
                </div>
              </div>
            </div>
            {/* Temperature */}
            <Show when={info()!.temperature_c != null}>
              <div class={styles.row}>
                <span class={styles.label}>Temp</span>
                <span class={styles.tempValue + ' ' + tempClass(info()!.temperature_c!)}>
                  {info()!.temperature_c!.toFixed(1)} °C
                </span>
              </div>
            </Show>
            {/* Uptime */}
            <div class={styles.row}>
              <span class={styles.label}>Uptime</span>
              <span class={styles.value}>{formatUptime(info()!.uptime_secs)}</span>
            </div>
          </div>
        </Show>
        <div class={styles.divider} />

        {/* Latency Section */}
        <div class={styles.section}>
          <span class={styles.sectionTitle}>Latency</span>
          <div class={styles.row}>
            <span class={styles.label}>Round-trip</span>
            <span
              class={styles.value}
              style={{ color: wsLatency() < 0 ? 'var(--text-muted)' : wsLatency() > 100 ? 'var(--accent-red)' : wsLatency() > 50 ? 'var(--accent-yellow, #e8a838)' : 'var(--accent-green, #4caf50)' }}
            >
              {wsLatency() < 0 ? '—' : `${wsLatency()} ms`}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SystemInfoPanel;
