import type { ClientMessage, ServerMessage } from './types';

export type MessageHandler = (msg: ServerMessage) => void;

/** WebSocket client with automatic reconnection and state sync */
export class WsClient {
  private ws: WebSocket | null = null;
  private url: string;
  private handlers: Set<MessageHandler> = new Set();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectDelay = 1000;
  private maxReconnectDelay = 10000;
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private _connected = false;

  constructor(url?: string) {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    this.url = url || `${proto}//${location.host}/ws`;
  }

  get connected(): boolean {
    return this._connected;
  }

  /** Subscribe to incoming messages */
  onMessage(handler: MessageHandler): () => void {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }

  /** Connect to the WebSocket server */
  connect(): void {
    if (this.ws) return;

    try {
      this.ws = new WebSocket(this.url);
    } catch {
      this.scheduleReconnect();
      return;
    }

    this.ws.onopen = () => {
      this._connected = true;
      this.reconnectDelay = 1000;
      console.log('[WS] Connected');

      // Request full state sync on (re)connect
      this.send({ type: 'RequestSync' });

      // Start keepalive pings
      this.pingInterval = setInterval(() => {
        this.send({ type: 'Ping' });
      }, 15000);
    };

    this.ws.onmessage = (event) => {
      let msg: ServerMessage;
      try {
        msg = JSON.parse(event.data);
      } catch (e) {
        console.warn('[WS] Failed to parse message:', e);
        return;
      }
      for (const handler of this.handlers) {
        try {
          handler(msg);
        } catch (e) {
          console.error('[WS] Handler error:', e);
        }
      }
    };

    this.ws.onclose = () => {
      this._connected = false;
      this.ws = null;
      this.clearPing();
      console.log('[WS] Disconnected');
      this.scheduleReconnect();
    };

    this.ws.onerror = () => {
      // onclose will fire after this
    };
  }

  /** Send a message to the server */
  send(msg: ClientMessage): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  /** Send a real-time command */
  sendRT(command: string): void {
    this.send({ type: 'RealtimeCommand', data: { command } });
  }

  /** Send a jog command */
  sendJog(axes: Record<string, number | undefined>, feed = 1000, incremental = true): void {
    this.send({
      type: 'Jog',
      data: { ...axes, feed, incremental },
    });
  }

  /** Send a console command */
  sendConsole(text: string): void {
    this.send({ type: 'ConsoleSend', data: text });
  }

  /** Send job control action */
  sendJobControl(action: 'Start' | 'Pause' | 'Resume' | 'Stop'): void {
    this.send({ type: 'JobControl', data: action });
  }

  /** Request full state sync */
  requestSync(): void {
    this.send({ type: 'RequestSync' });
  }

  /** Request port list */
  requestPorts(): void {
    this.send({ type: 'RequestPortList' });
  }

  /** Disconnect and stop reconnecting */
  disconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.clearPing();
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    this._connected = false;
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;
    console.log(`[WS] Reconnecting in ${this.reconnectDelay}ms...`);
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, this.reconnectDelay);
    this.reconnectDelay = Math.min(this.reconnectDelay * 1.5, this.maxReconnectDelay);
  }

  private clearPing(): void {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
  }
}

/** Singleton WebSocket client */
export const ws = new WsClient();
