import { createSignal, batch } from 'solid-js';
import type {
  MachineSnapshot,
  Position,
  Overrides,
  JobProgress,
  ConsoleEntry,
  ConnectionState,
  FileInfo,
  PortInfo,
  AlarmNotification,
  ServerMessage,
  GCodeFileInfo,
} from './types';
import { ws } from './ws';

// ── Machine State Signals ──

const defaultPos: Position = { x: 0, y: 0, z: 0 };
const defaultOverrides: Overrides = { feed: 100, rapids: 100, spindle: 100 };

export const [machineState, setMachineState] = createSignal<string>('Idle');
export const [machinePos, setMachinePos] = createSignal<Position>(defaultPos);
export const [workPos, setWorkPos] = createSignal<Position>(defaultPos);
export const [feedRate, setFeedRate] = createSignal(0);
export const [spindleSpeed, setSpindleSpeed] = createSignal(0);
export const [overrides, setOverrides] = createSignal<Overrides>(defaultOverrides);
export const [lineNumber, setLineNumber] = createSignal(0);
export const [connected, setConnected] = createSignal(false);

// ── Job Progress ──

export const [jobProgress, setJobProgress] = createSignal<JobProgress | null>(null);

// ── Console ──

const MAX_CONSOLE_LINES = 500;
export const [consoleLines, setConsoleLines] = createSignal<ConsoleEntry[]>([]);

export function addConsoleLine(entry: ConsoleEntry) {
  setConsoleLines((prev) => {
    if (prev.length >= MAX_CONSOLE_LINES) {
      return [...prev.slice(1), entry];
    }
    return [...prev, entry];
  });
}

// ── Connection ──

export const [connectionState, setConnectionState] = createSignal<ConnectionState>({
  connected: false,
});

// ── Files ──

export const [files, setFiles] = createSignal<FileInfo[]>([]);

// ── Ports ──

export const [ports, setPorts] = createSignal<PortInfo[]>([]);

// ── Alarm ──

export const [alarm, setAlarm] = createSignal<AlarmNotification | null>(null);

// ── G-code for 3D viewer ──

export const [gcodeFile, setGcodeFile] = createSignal<GCodeFileInfo | null>(null);

// ── WebSocket connected ──

export const [wsConnected, setWsConnected] = createSignal(false);

// ── State name helper ──

function extractStateName(state: unknown): string {
  if (typeof state === 'string') return state;
  if (state && typeof state === 'object') return Object.keys(state)[0] || 'Unknown';
  return 'Unknown';
}

// ── Message handler: updates all signals from server messages ──

function handleServerMessage(msg: ServerMessage): void {
  switch (msg.type) {
    case 'MachineState': {
      const s = msg.data;
      batch(() => {
        setMachineState(extractStateName(s.state));
        setMachinePos(s.machine_pos);
        setWorkPos(s.work_pos);
        setFeedRate(s.feed_rate);
        setSpindleSpeed(s.spindle_speed);
        setOverrides(s.overrides);
        setLineNumber(s.line_number);
        setConnected(s.connected);
      });
      break;
    }
    case 'JobProgress':
      setJobProgress(msg.data);
      break;
    case 'ConsoleOutput':
      addConsoleLine(msg.data);
      break;
    case 'ConnectionChanged':
      batch(() => {
        setConnectionState(msg.data);
        setConnected(msg.data.connected);
      });
      break;
    case 'FileListUpdated':
      setFiles(msg.data);
      break;
    case 'Error':
      addConsoleLine({
        direction: 'System',
        text: `ERROR: ${msg.data.message}`,
        timestamp: Date.now(),
      });
      break;
    case 'StateSync': {
      const sync = msg.data;
      batch(() => {
        setMachineState(extractStateName(sync.machine.state));
        setMachinePos(sync.machine.machine_pos);
        setWorkPos(sync.machine.work_pos);
        setFeedRate(sync.machine.feed_rate);
        setSpindleSpeed(sync.machine.spindle_speed);
        setOverrides(sync.machine.overrides);
        setLineNumber(sync.machine.line_number);
        setConnected(sync.machine.connected);
        setConnectionState(sync.connection);
        if (sync.job) setJobProgress(sync.job);
        setFiles(sync.files);
      });
      addConsoleLine({
        direction: 'System',
        text: 'State synchronized',
        timestamp: Date.now(),
      });
      break;
    }
    case 'Alarm':
      batch(() => {
        setAlarm(msg.data);
        addConsoleLine({
          direction: 'System',
          text: `ALARM:${msg.data.code} - ${msg.data.message}`,
          timestamp: Date.now(),
        });
      });
      break;
    case 'GCodeLoaded':
      setGcodeFile(msg.data);
      break;
    case 'PortList':
      setPorts(msg.data);
      break;
    case 'Pong':
      break;
  }
}

// ── Initialize: connect WebSocket and wire up handler ──

let initialized = false;
let wsCheckInterval: ReturnType<typeof setInterval> | undefined;

export function initStore(): void {
  if (initialized) return;
  initialized = true;
  ws.onMessage(handleServerMessage);
  ws.connect();

  // Track WS connection status
  wsCheckInterval = setInterval(() => setWsConnected(ws.connected), 1000);
}
