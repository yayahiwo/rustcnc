import { Component, createSignal, For, onCleanup, createEffect } from 'solid-js';
import { consoleLines, setConsoleLines } from '../../lib/store';
import { ws } from '../../lib/ws';
import type { ConsoleEntry } from '../../lib/types';
import styles from './Console.module.css';

const Console: Component = () => {
  const [input, setInput] = createSignal('');
  const [history, setHistory] = createSignal<string[]>([]);
  const [historyIdx, setHistoryIdx] = createSignal(-1);
  const [height, setHeight] = createSignal(250);
  let outputRef: HTMLDivElement | undefined;
  let containerRef: HTMLDivElement | undefined;

  // Auto-scroll to bottom on new messages (only if user is near bottom)
  createEffect(() => {
    consoleLines(); // track dependency
    if (outputRef) {
      const isAtBottom = outputRef.scrollHeight - outputRef.scrollTop - outputRef.clientHeight < 50;
      if (isAtBottom) {
        outputRef.scrollTop = outputRef.scrollHeight;
      }
    }
  });

  const handleSend = () => {
    const text = input().trim();
    if (!text) return;
    ws.sendConsole(text);
    setHistory((prev) => [text, ...prev].slice(0, 50));
    setHistoryIdx(-1);
    setInput('');
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSend();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const hist = history();
      const idx = historyIdx();
      if (idx < hist.length - 1) {
        const newIdx = idx + 1;
        setHistoryIdx(newIdx);
        setInput(hist[newIdx]);
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      const idx = historyIdx();
      if (idx > 0) {
        const newIdx = idx - 1;
        setHistoryIdx(newIdx);
        setInput(history()[newIdx]);
      } else if (idx === 0) {
        setHistoryIdx(-1);
        setInput('');
      }
    }
  };

  const renderConsoleText = (text: string) => {
    // Setting lines: "$110=8000.000 (X-axis maximum rate, mm/min)"
    const m = text.match(/^(\$\d+=\S+)\s+(\(.+\))$/);
    if (m) {
      return <>{m[1]} <span class={styles.settingDesc}>{m[2]}</span></>;
    }
    return text;
  };

  const handleResizeStart = (e: MouseEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startH = height();
    const onMove = (ev: MouseEvent) => {
      const newH = Math.max(100, startH + (ev.clientY - startY));
      setHeight(newH);
    };
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  };

  const dirClass = (entry: ConsoleEntry) => {
    switch (entry.direction) {
      case 'Sent': return styles.sent;
      case 'Received': return styles.received;
      case 'System': return styles.system;
      default: return '';
    }
  };

  return (
    <div class={'panel ' + styles.console} ref={containerRef} style={{ height: `${height()}px` }}>
      <div class="panel-header">Console</div>
      <div class={styles.output} ref={outputRef}>
        <For each={consoleLines()}>
          {(entry) => (
            <div class={styles.line + ' ' + dirClass(entry)}>
              <span class={styles.dir}>
                {entry.direction === 'Sent' ? '>' : entry.direction === 'Received' ? '<' : '#'}
              </span>
              <span class={styles.text}>{renderConsoleText(entry.text)}</span>
            </div>
          )}
        </For>
      </div>
      <div class={styles.inputRow}>
        <input
          class={styles.input}
          type="text"
          placeholder="G-code command..."
          value={input()}
          onInput={(e) => setInput(e.currentTarget.value)}
          onKeyDown={handleKeyDown}
        />
        <button class={styles.clearBtn} onClick={() => setConsoleLines([])}>
          Clear
        </button>
        <button class={styles.sendBtn} onClick={handleSend}>
          Send
        </button>
      </div>
      <div class={styles.resizeHandle} onMouseDown={handleResizeStart} />
    </div>
  );
};

export default Console;
