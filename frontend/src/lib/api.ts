const BASE = '/api';

async function request<T>(path: string, opts?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...opts,
  });
  if (!res.ok) throw new Error(`API ${res.status}: ${res.statusText}`);
  return res.json();
}

export const api = {
  // Connection
  listPorts: () => request<{ path: string; manufacturer?: string; product?: string }[]>('/ports'),
  connect: (port: string, baud_rate = 115200) =>
    request('/connect', { method: 'POST', body: JSON.stringify({ port, baud_rate }) }),
  disconnect: () => request('/disconnect', { method: 'POST' }),

  // Files
  listFiles: () => request<{ id: string; name: string; size_bytes: number; line_count: number }[]>('/files'),
  uploadFile: async (file: File) => {
    const form = new FormData();
    form.append('file', file);
    const res = await fetch(`${BASE}/files`, { method: 'POST', body: form });
    if (!res.ok) throw new Error(`Upload failed: ${res.statusText}`);
    return res.json();
  },
  deleteFile: (id: string) => fetch(`${BASE}/files/${id}`, { method: 'DELETE' }),
  loadFile: (id: string) => fetch(`${BASE}/files/${id}/load`, { method: 'POST' }),

  // Job control
  startJob: () => fetch(`${BASE}/job/start`, { method: 'POST' }),
  pauseJob: () => fetch(`${BASE}/job/pause`, { method: 'POST' }),
  resumeJob: () => fetch(`${BASE}/job/resume`, { method: 'POST' }),
  cancelJob: () => fetch(`${BASE}/job/cancel`, { method: 'POST' }),

  // Machine
  home: () => fetch(`${BASE}/machine/home`, { method: 'POST' }),
  unlock: () => fetch(`${BASE}/machine/unlock`, { method: 'POST' }),
  reset: () => fetch(`${BASE}/machine/reset`, { method: 'POST' }),
  sendCommand: (command: string) =>
    fetch(`${BASE}/machine/command`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ command }),
    }),

  // Settings
  getSettings: () => request('/settings'),
  getGrblSettings: () => fetch(`${BASE}/settings/grbl`, { method: 'GET' }),

  // System
  systemInfo: () => request<{ version: string; platform: string; connected: boolean }>('/system/info'),
};
