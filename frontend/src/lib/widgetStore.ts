import { createSignal } from 'solid-js';

// ── Types ──

export type WidgetId =
  | 'dro'
  | 'jogpad'
  | 'overrides'
  | 'toolpath-viewer'
  | 'gcode-viewer'
  | 'job-progress'
  | 'file-list'
  | 'console';

export interface LayoutState {
  columnCount: number;       // 1–4
  columnWidths: number[];    // e.g. [3, 6, 3], must sum to 12
  columns: WidgetId[][];     // one array per column
  collapsed: Record<string, boolean>;
  axisCount: number;         // 3–8 (grblHAL supports up to 8)
}

// ── Axis definitions (grblHAL order) ──

export interface AxisDef {
  name: string;           // display label
  key: string;            // Position property key
  color: string;          // CSS variable
}

export const ALL_AXES: AxisDef[] = [
  { name: 'X', key: 'x', color: 'var(--axis-x)' },
  { name: 'Y', key: 'y', color: 'var(--axis-y)' },
  { name: 'Z', key: 'z', color: 'var(--axis-z)' },
  { name: 'A', key: 'a', color: 'var(--axis-a)' },
  { name: 'B', key: 'b', color: 'var(--axis-b)' },
  { name: 'C', key: 'c', color: 'var(--axis-c)' },
  { name: 'U', key: 'u', color: 'var(--axis-u)' },
  { name: 'V', key: 'v', color: 'var(--axis-v)' },
];

// ── Widget Registry ──

export interface WidgetMeta {
  id: WidgetId;
  defaultColumn: number;     // 0-based column index (for 3-column default)
  flex?: number;
}

export const WIDGET_REGISTRY: WidgetMeta[] = [
  { id: 'dro', defaultColumn: 0 },
  { id: 'jogpad', defaultColumn: 0 },
  { id: 'overrides', defaultColumn: 0 },
  { id: 'toolpath-viewer', defaultColumn: 1, flex: 2 },
  { id: 'gcode-viewer', defaultColumn: 1, flex: 1 },
  { id: 'job-progress', defaultColumn: 2 },
  { id: 'file-list', defaultColumn: 2 },
  { id: 'console', defaultColumn: 2, flex: 1 },
];

const KNOWN_IDS = new Set<string>(WIDGET_REGISTRY.map((w) => w.id));

// ── Default presets ──

const DEFAULT_WIDTHS: Record<number, number[]> = {
  1: [12],
  2: [4, 8],
  3: [3, 6, 3],
  4: [2, 4, 4, 2],
};

const DEFAULT_LAYOUT: LayoutState = {
  columnCount: 3,
  columnWidths: [3, 6, 3],
  columns: [
    ['dro', 'jogpad', 'overrides'],
    ['toolpath-viewer', 'gcode-viewer'],
    ['job-progress', 'file-list', 'console'],
  ],
  collapsed: {},
  axisCount: 3,
};

const STORAGE_KEY = 'rustcnc-widget-layout';

// ── Persistence ──

function loadLayout(): LayoutState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return structuredClone(DEFAULT_LAYOUT);
    const saved = JSON.parse(raw) as LayoutState;
    return migrateLayout(saved);
  } catch {
    return structuredClone(DEFAULT_LAYOUT);
  }
}

function migrateLayout(saved: LayoutState): LayoutState {
  // Handle old format with named columns (left/center/right)
  if (saved.columns && !Array.isArray(saved.columns)) {
    const old = saved.columns as unknown as Record<string, WidgetId[]>;
    saved.columns = [
      old['left'] || [],
      old['center'] || [],
      old['right'] || [],
    ];
    saved.columnCount = 3;
    saved.columnWidths = [3, 6, 3];
  }

  // Ensure columnCount and columnWidths exist
  if (!saved.columnCount || saved.columnCount < 1 || saved.columnCount > 4) {
    saved.columnCount = 3;
  }
  if (!Array.isArray(saved.columnWidths) || saved.columnWidths.length !== saved.columnCount) {
    saved.columnWidths = DEFAULT_WIDTHS[saved.columnCount];
  }
  // Ensure widths sum to 12
  const sum = saved.columnWidths.reduce((a, b) => a + b, 0);
  if (sum !== 12) {
    saved.columnWidths = DEFAULT_WIDTHS[saved.columnCount];
  }

  // Ensure columns array matches columnCount
  if (!Array.isArray(saved.columns)) {
    saved.columns = [];
  }
  while (saved.columns.length < saved.columnCount) {
    saved.columns.push([]);
  }
  // If there are extra columns, merge their widgets into the last valid column
  while (saved.columns.length > saved.columnCount) {
    const extra = saved.columns.pop()!;
    saved.columns[saved.columns.length - 1].push(...extra);
  }

  // Deduplicate and validate widget IDs
  const present = new Set<string>();
  for (let i = 0; i < saved.columns.length; i++) {
    if (!Array.isArray(saved.columns[i])) {
      saved.columns[i] = [];
    }
    saved.columns[i] = saved.columns[i].filter((id) => {
      if (KNOWN_IDS.has(id) && !present.has(id)) {
        present.add(id);
        return true;
      }
      return false;
    });
  }

  // Add missing widgets to their default column (clamped to available columns)
  for (const meta of WIDGET_REGISTRY) {
    if (!present.has(meta.id)) {
      const col = Math.min(meta.defaultColumn, saved.columnCount - 1);
      saved.columns[col].push(meta.id);
    }
  }

  // Ensure axisCount is valid
  if (!saved.axisCount || saved.axisCount < 3 || saved.axisCount > 8) {
    saved.axisCount = 3;
  }

  // Clean collapsed state
  if (saved.collapsed) {
    for (const key of Object.keys(saved.collapsed)) {
      if (!KNOWN_IDS.has(key)) {
        delete saved.collapsed[key];
      }
    }
  } else {
    saved.collapsed = {};
  }

  return saved;
}

function saveLayout(state: LayoutState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // localStorage full or unavailable — silently ignore
  }
}

// ── Signals ──

const [layout, setLayout] = createSignal<LayoutState>(loadLayout());

export { layout };

function updateLayout(fn: (prev: LayoutState) => LayoutState): void {
  setLayout((prev) => {
    const next = fn(structuredClone(prev));
    saveLayout(next);
    return next;
  });
}

// ── Mutations ──

export function toggleCollapsed(id: WidgetId): void {
  updateLayout((state) => {
    state.collapsed[id] = !state.collapsed[id];
    return state;
  });
}

export function moveWidget(
  widgetId: WidgetId,
  toColumn: number,
  toIndex: number,
): void {
  updateLayout((state) => {
    // Remove from current column
    for (let i = 0; i < state.columns.length; i++) {
      const idx = state.columns[i].indexOf(widgetId);
      if (idx !== -1) {
        state.columns[i].splice(idx, 1);
        break;
      }
    }
    // Insert at target position
    const target = state.columns[toColumn];
    const clampedIndex = Math.min(toIndex, target.length);
    target.splice(clampedIndex, 0, widgetId);
    return state;
  });
}

export function setColumnCount(count: number): void {
  if (count < 1 || count > 4) return;
  updateLayout((state) => {
    if (count === state.columnCount) return state;

    // Adjust columns array
    if (count > state.columnCount) {
      // Adding columns — new columns start empty
      while (state.columns.length < count) {
        state.columns.push([]);
      }
    } else {
      // Removing columns — merge extras into the last remaining column
      while (state.columns.length > count) {
        const extra = state.columns.pop()!;
        state.columns[state.columns.length - 1].push(...extra);
      }
    }

    state.columnCount = count;
    state.columnWidths = [...DEFAULT_WIDTHS[count]];
    return state;
  });
}

export function setColumnWidth(colIndex: number, newWidth: number): void {
  updateLayout((state) => {
    if (colIndex < 0 || colIndex >= state.columnCount) return state;
    if (newWidth < 1) return state;

    const oldWidth = state.columnWidths[colIndex];
    const delta = newWidth - oldWidth;
    if (delta === 0) return state;

    // Find a neighbor column to absorb the delta
    // Try the column to the right first, then left
    const neighbor = colIndex < state.columnCount - 1 ? colIndex + 1 : colIndex - 1;
    if (neighbor < 0 || neighbor >= state.columnCount) return state;

    const neighborWidth = state.columnWidths[neighbor] - delta;
    if (neighborWidth < 1) return state;

    state.columnWidths[colIndex] = newWidth;
    state.columnWidths[neighbor] = neighborWidth;
    return state;
  });
}

export function setAxisCount(count: number): void {
  if (count < 3 || count > 8) return;
  updateLayout((state) => {
    state.axisCount = count;
    return state;
  });
}

export function getWidgetMeta(id: WidgetId): WidgetMeta | undefined {
  return WIDGET_REGISTRY.find((w) => w.id === id);
}
