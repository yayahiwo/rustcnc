// ── Types mirroring rustcnc-core/src/ws_protocol.rs ──

export type MachineState =
  | 'Idle'
  | { Hold: number }
  | 'Run'
  | 'Jog'
  | { Alarm: number }
  | { Door: number }
  | 'Check'
  | 'Home'
  | 'Sleep'
  | 'Tool';

export interface Position {
  x: number;
  y: number;
  z: number;
  a?: number;
  b?: number;
  c?: number;
  u?: number;
  v?: number;
}

export interface Overrides {
  feed: number;
  rapids: number;
  spindle: number;
}

export interface AccessoryState {
  spindle_cw: boolean;
  spindle_ccw: boolean;
  flood_coolant: boolean;
  mist_coolant: boolean;
}

export interface InputPins {
  limit_x: boolean;
  limit_y: boolean;
  limit_z: boolean;
  limit_a: boolean;
  limit_b: boolean;
  limit_c: boolean;
  limit_u: boolean;
  limit_v: boolean;
  probe: boolean;
  door: boolean;
  hold: boolean;
  soft_reset: boolean;
  cycle_start: boolean;
  estop: boolean;
}

export interface BufferState {
  planner_blocks_available: number;
  rx_bytes_available: number;
}

export type FirmwareType = 'Grbl' | 'GrblHal' | 'Unknown';

export interface MachineSnapshot {
  state: MachineState;
  machine_pos: Position;
  work_pos: Position;
  feed_rate: number;
  spindle_speed: number;
  overrides: Overrides;
  accessories: AccessoryState;
  input_pins: InputPins;
  buffer: BufferState;
  line_number: number;
  connected: boolean;
  firmware: FirmwareType;
}

// ── Server → Client messages ──

export interface JobProgress {
  file_name: string;
  current_line: number;
  total_lines: number;
  percent_complete: number;
  elapsed_secs: number;
  estimated_remaining_secs?: number;
  state: JobState;
}

export type JobState = 'Idle' | 'Running' | 'Paused' | 'Completed' | 'Error' | 'Cancelled';

export interface ConsoleEntry {
  direction: 'Sent' | 'Received' | 'System';
  text: string;
  timestamp: number;
}

export interface ConnectionState {
  connected: boolean;
  port?: string;
  firmware?: string;
  version?: string;
}

export interface FileInfo {
  id: string;
  name: string;
  size_bytes: number;
  line_count: number;
  loaded_at: string;
}

export interface FullStateSync {
  machine: MachineSnapshot;
  connection: ConnectionState;
  job?: JobProgress;
  files: FileInfo[];
}

export interface ErrorNotification {
  code?: number;
  message: string;
  source: string;
}

export interface AlarmNotification {
  code: number;
  message: string;
}

export interface PortInfo {
  path: string;
  manufacturer?: string;
  product?: string;
}

export interface ArcData {
  i: number;
  j: number;
  k: number;
  plane: number;
}

export interface GCodeLineInfo {
  line_num: number;
  text: string;
  move_type?: string;
  endpoint?: number[];
  arc?: ArcData;
}

export interface GCodeFileInfo {
  id: string;
  name: string;
  lines: GCodeLineInfo[];
  bounding_box?: [number[], number[]];
}

export type ServerMessage =
  | { type: 'MachineState'; data: MachineSnapshot }
  | { type: 'JobProgress'; data: JobProgress }
  | { type: 'ConsoleOutput'; data: ConsoleEntry }
  | { type: 'ConnectionChanged'; data: ConnectionState }
  | { type: 'FileListUpdated'; data: FileInfo[] }
  | { type: 'Error'; data: ErrorNotification }
  | { type: 'StateSync'; data: FullStateSync }
  | { type: 'Alarm'; data: AlarmNotification }
  | { type: 'GCodeLoaded'; data: GCodeFileInfo }
  | { type: 'PortList'; data: PortInfo[] }
  | { type: 'Pong' }
  | { type: 'SystemAlert'; data: string | null }
  | { type: 'SystemInfo'; data: SystemInfo };

export interface SystemInfo {
  cpu_load: [number, number, number];
  memory_total_mb: number;
  memory_used_mb: number;
  disk_total_gb: number;
  disk_used_gb: number;
  temperature_c?: number;
  uptime_secs: number;
  firmware_version?: string;
  serial_port?: string;
  connection_uptime_secs: number;
  grbl_info?: Record<string, string>;
}

// ── Pause condition ──

export type PauseCondition =
  | 'EndOfLayer'
  | { AtZDepth: { z: number } };

// ── Client → Server messages ──

export type JobControlAction =
  | { action: 'Start'; start_line?: number; stop_line?: number }
  | { action: 'Pause' }
  | { action: 'Resume' }
  | { action: 'Stop' };

export type ClientMessage =
  | { type: 'RealtimeCommand'; data: { command: string } }
  | { type: 'Jog'; data: JogCommand }
  | { type: 'ConsoleSend'; data: string }
  | { type: 'RequestSync' }
  | { type: 'Ping' }
  | { type: 'JobControl'; data: JobControlAction }
  | { type: 'Connect'; data: { port: string; baud_rate: number } }
  | { type: 'Disconnect' }
  | { type: 'RequestPortList' }
  | { type: 'SchedulePause'; data: PauseCondition | null };

export interface JogCommand {
  x?: number;
  y?: number;
  z?: number;
  a?: number;
  b?: number;
  c?: number;
  u?: number;
  v?: number;
  feed: number;
  incremental: boolean;
  distance?: number;
}

// ── Helper to extract state name ──

export function machineStateName(state: MachineState): string {
  if (typeof state === 'string') return state;
  const key = Object.keys(state)[0];
  return key;
}

export function machineStateSubcode(state: MachineState): number | undefined {
  if (typeof state === 'string') return undefined;
  return Object.values(state)[0] as number;
}

export function isAlarm(state: MachineState): boolean {
  return typeof state === 'object' && 'Alarm' in state;
}

export function isHold(state: MachineState): boolean {
  return typeof state === 'object' && 'Hold' in state;
}
