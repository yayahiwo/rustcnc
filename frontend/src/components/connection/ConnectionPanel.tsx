import { Component, createSignal, onMount, Show } from 'solid-js';
import { connectionState, ports } from '../../lib/store';
import { ws } from '../../lib/ws';
import PortSelector from './PortSelector';
import styles from './ConnectionPanel.module.css';

const ConnectionPanel: Component = () => {
  const [selectedPort, setSelectedPort] = createSignal('');
  const [baudRate, setBaudRate] = createSignal(115200);

  onMount(() => {
    ws.requestPorts();
  });

  const handleConnect = () => {
    const port = selectedPort();
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
    <div class={styles.panel}>
      <div class={styles.header}>Connection</div>
      <div class={styles.body}>
        <Show
          when={!isConnected()}
          fallback={
            <div class={styles.connected}>
              <div class={styles.connInfo}>
                <span class={styles.dot} />
                <span>{connectionState().port || 'Connected'}</span>
              </div>
              <Show when={connectionState().firmware}>
                <span class={styles.firmware}>
                  {connectionState().firmware} {connectionState().version || ''}
                </span>
              </Show>
              <button class={styles.disconnectBtn} onClick={handleDisconnect}>
                Disconnect
              </button>
            </div>
          }
        >
          <PortSelector
            ports={ports()}
            selected={selectedPort()}
            onSelect={setSelectedPort}
            onRefresh={handleRefresh}
          />
          <div class={styles.baudRow}>
            <label class={styles.label}>Baud</label>
            <select
              value={baudRate()}
              onChange={(e) => setBaudRate(Number(e.currentTarget.value))}
              class={styles.select}
            >
              <option value={9600}>9600</option>
              <option value={19200}>19200</option>
              <option value={38400}>38400</option>
              <option value={57600}>57600</option>
              <option value={115200}>115200</option>
              <option value={230400}>230400</option>
            </select>
          </div>
          <button
            class={styles.connectBtn}
            onClick={handleConnect}
            disabled={!selectedPort()}
          >
            Connect
          </button>
        </Show>
      </div>
    </div>
  );
};

export default ConnectionPanel;
