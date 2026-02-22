import { Component, createSignal, For, onMount, Show } from 'solid-js';
import { connectionState, ports } from '../../lib/store';
import { ws } from '../../lib/ws';
import styles from './ConnectionPanel.module.css';

const BAUD_RATES = [9600, 19200, 38400, 57600, 115200, 250000];

const ConnectionPanel: Component = () => {
  const [selectedPort, setSelectedPort] = createSignal('');
  const [manualPort, setManualPort] = createSignal('');
  const [baudRate, setBaudRate] = createSignal(115200);

  const effectivePort = () => manualPort() || selectedPort();

  onMount(() => {
    ws.requestPorts();
  });

  const handleConnect = () => {
    const port = effectivePort();
    if (!port) return;
    ws.send({ type: 'Connect', data: { port, baud_rate: baudRate() } });
  };

  const handleDisconnect = () => {
    ws.send({ type: 'Disconnect' });
  };

  const handleRefresh = () => {
    ws.requestPorts();
  };

  const isConnected = () => connectionState().connected;

  return (
    <div class="panel">
      <div class="panel-header">
        <span>Connection</span>
        <Show when={isConnected()}>
          <span class={styles.statusDot} />
        </Show>
      </div>
      <div class={styles.body}>
        <div class={styles.row}>
          <label class={styles.label}>Port</label>
          <select
            class={styles.select}
            value={selectedPort()}
            onChange={(e) => setSelectedPort(e.currentTarget.value)}
            disabled={isConnected()}
          >
            <option value="">Select port...</option>
            <For each={ports()}>
              {(port) => (
                <option value={port.path}>
                  {port.path}{port.manufacturer ? ` (${port.manufacturer})` : ''}
                </option>
              )}
            </For>
          </select>
          <button
            class={styles.refreshBtn}
            onClick={handleRefresh}
            title="Refresh ports"
            disabled={isConnected()}
          >
            &#x21bb;
          </button>
        </div>

        <div class={styles.row}>
          <label class={styles.label}>Path</label>
          <input
            class={styles.input}
            type="text"
            placeholder="/dev/ttyUSB0"
            value={manualPort()}
            onInput={(e) => setManualPort(e.currentTarget.value)}
            disabled={isConnected()}
          />
        </div>

        <div class={styles.row}>
          <label class={styles.label}>Baud</label>
          <select
            class={styles.select}
            value={baudRate()}
            onChange={(e) => setBaudRate(Number(e.currentTarget.value))}
            disabled={isConnected()}
          >
            <For each={BAUD_RATES}>
              {(rate) => <option value={rate}>{rate}</option>}
            </For>
          </select>
        </div>

        <Show when={isConnected() && connectionState().firmware}>
          <div class={styles.firmware}>
            {connectionState().firmware} {connectionState().version || ''}
          </div>
        </Show>

        <div class={styles.buttons}>
          <button
            class={styles.connectBtn}
            onClick={handleConnect}
            disabled={!effectivePort() || isConnected()}
          >
            Connect
          </button>
          <button
            class={styles.disconnectBtn}
            onClick={handleDisconnect}
            disabled={!isConnected()}
          >
            Disconnect
          </button>
        </div>
      </div>
    </div>
  );
};

export default ConnectionPanel;
