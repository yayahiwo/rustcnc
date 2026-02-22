import { createSignal } from 'solid-js';
import { api } from './api';
import { ws } from './ws';

export interface AuthStatus {
  enabled: boolean;
  authenticated: boolean;
  username?: string | null;
}

export const [authReady, setAuthReady] = createSignal(false);
export const [authEnabled, setAuthEnabled] = createSignal(false);
export const [authenticated, setAuthenticated] = createSignal(true);
export const [authUsername, setAuthUsername] = createSignal<string | null>(null);

export async function refreshAuthStatus(): Promise<AuthStatus> {
  try {
    const st = await api.authStatus();
    setAuthEnabled(!!st.enabled);
    setAuthenticated(!!st.authenticated);
    setAuthUsername(st.username ?? null);
    setAuthReady(true);
    if (st.enabled && !st.authenticated) {
      ws.disconnect();
    }
    return st;
  } catch (e) {
    setAuthReady(true);
    const msg = e instanceof Error ? e.message : String(e);
    // Back-compat: server without auth endpoints.
    if (msg.startsWith('API 404')) {
      setAuthEnabled(false);
      setAuthenticated(true);
      setAuthUsername(null);
      return { enabled: false, authenticated: true, username: null };
    }
    // Transient error (server down, network blip): keep last known state.
    return { enabled: authEnabled(), authenticated: authenticated(), username: authUsername() };
  }
}

export async function login(username: string, password: string): Promise<void> {
  await api.authLogin(username, password);
  const st = await refreshAuthStatus();
  if (st.enabled && st.authenticated) {
    ws.connect();
  }
}

export async function logout(): Promise<void> {
  try {
    await api.authLogout();
  } finally {
    ws.disconnect();
    await refreshAuthStatus();
  }
}
