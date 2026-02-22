import { Component, Show, createSignal } from 'solid-js';
import { login } from '../../lib/auth';
import styles from './LoginOverlay.module.css';

const LoginOverlay: Component = () => {
  const [username, setUsername] = createSignal('');
  const [password, setPassword] = createSignal('');
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  const onSubmit = async (e: Event) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await login(username().trim(), password());
      setPassword('');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.startsWith('API 401')) {
        setError('Invalid username or password');
      } else {
        setError('Login failed');
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class={styles.overlay}>
      <form class={styles.card} onSubmit={onSubmit}>
        <div class={styles.title}>Login required</div>
        <div class={styles.subtitle}>Enter the credentials set during installation.</div>

        <div class={styles.field}>
          <div class={styles.label}>Username</div>
          <input
            class={styles.input}
            value={username()}
            onInput={(e) => setUsername(e.currentTarget.value)}
            autocomplete="username"
            required
          />
        </div>

        <div class={styles.field}>
          <div class={styles.label}>Password</div>
          <input
            class={styles.input}
            type="password"
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
            autocomplete="current-password"
            required
          />
        </div>

        <Show when={error()}>
          <div class={styles.error}>{error()}</div>
        </Show>

        <div class={styles.actions}>
          <button class={styles.btn + ' ' + styles.btnPrimary} type="submit" disabled={busy()}>
            {busy() ? 'Logging in…' : 'Login'}
          </button>
        </div>
      </form>
    </div>
  );
};

export default LoginOverlay;

