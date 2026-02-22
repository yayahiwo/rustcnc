import { Component, createSignal, createEffect, For, Show } from 'solid-js';
import { machineState, connected, jobProgress } from '../../lib/store';
import { ws } from '../../lib/ws';
import styles from './MacroPanel.module.css';

interface Macro {
  id: number;
  name: string;
  gcode: string;
}

const STORAGE_KEY = 'rustcnc-macros';

function loadMacros(): Macro[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (m: any) =>
        typeof m.id === 'number' &&
        typeof m.name === 'string' &&
        typeof m.gcode === 'string'
    );
  } catch {
    return [];
  }
}

const MacroPanel: Component = () => {
  const [macros, setMacros] = createSignal<Macro[]>(loadMacros());
  const [editing, setEditing] = createSignal<Macro | null>(null);
  const [editName, setEditName] = createSignal('');
  const [editGcode, setEditGcode] = createSignal('');

  createEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(macros()));
  });

  const canRun = () => {
    const s = machineState();
    return connected() && (s === 'Idle' || s === 'Jog');
  };

  const hasActiveJob = () => {
    const s = jobProgress()?.state;
    return s === 'Running' || s === 'Paused';
  };

  const handleRun = (macro: Macro) => {
    if (!canRun() || hasActiveJob()) return;
    const lines = macro.gcode
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l.length > 0);
    for (const line of lines) {
      ws.sendConsole(line);
    }
  };

  const handleAdd = () => {
    setEditing({ id: 0, name: '', gcode: '' });
    setEditName('');
    setEditGcode('');
  };

  const handleEdit = (macro: Macro) => {
    setEditing(macro);
    setEditName(macro.name);
    setEditGcode(macro.gcode);
  };

  const handleSave = () => {
    const name = editName().trim();
    const gcode = editGcode().trim();
    if (!name || !gcode) return;

    const current = editing();
    if (!current) return;

    if (current.id === 0) {
      const nextId = Math.max(...macros().map((m) => m.id), 0) + 1;
      setMacros([...macros(), { id: nextId, name, gcode }]);
    } else {
      setMacros(macros().map((m) => (m.id === current.id ? { ...m, name, gcode } : m)));
    }
    setEditing(null);
  };

  const handleCancel = () => {
    setEditing(null);
  };

  const handleDelete = (id: number) => {
    setMacros(macros().filter((m) => m.id !== id));
  };

  return (
    <div class="panel">
      <div class="panel-header">Macros</div>
      <div class={styles.body}>
        <Show when={macros().length === 0 && !editing()}>
          <div class={styles.empty}>No macros yet</div>
        </Show>
        <For each={macros()}>
          {(macro) => (
            <div class={styles.macroRow}>
              <span class={styles.macroName} title={macro.gcode} onClick={() => handleEdit(macro)}>
                {macro.name}
              </span>
              <button
                class={styles.runBtn}
                onClick={() => handleRun(macro)}
                disabled={!canRun() || hasActiveJob()}
                title="Run macro"
              >
                Run
              </button>
              <button
                class={styles.editBtn}
                onClick={() => handleEdit(macro)}
                title="Edit macro"
              >
                &#9998;
              </button>
              <button
                class={styles.deleteBtn}
                onClick={() => handleDelete(macro.id)}
                title="Delete macro"
              >
                &times;
              </button>
            </div>
          )}
        </For>
        <Show when={editing()}>
          <div class={styles.editor}>
            <input
              class={styles.editorInput}
              type="text"
              placeholder="Macro name"
              value={editName()}
              onInput={(e) => setEditName(e.currentTarget.value)}
            />
            <textarea
              class={styles.editorTextarea}
              placeholder="G-code commands (one per line)"
              value={editGcode()}
              onInput={(e) => setEditGcode(e.currentTarget.value)}
              rows={4}
            />
            <div class={styles.editorButtons}>
              <button class={styles.cancelBtn} onClick={handleCancel}>
                Cancel
              </button>
              <button class={styles.saveBtn} onClick={handleSave}>
                Save
              </button>
            </div>
          </div>
        </Show>
        <Show when={!editing()}>
          <button class={styles.addBtn} onClick={handleAdd}>
            + Add Macro
          </button>
        </Show>
      </div>
    </div>
  );
};

export default MacroPanel;
