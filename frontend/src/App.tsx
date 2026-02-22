import { Component, Show } from 'solid-js';
import Dashboard from './components/layout/Dashboard';
import LoginOverlay from './components/auth/LoginOverlay';
import { authEnabled, authReady, authenticated } from './lib/auth';

const App: Component = () => {
  return (
    <>
      <Dashboard />
      <Show when={authReady() && authEnabled() && !authenticated()}>
        <LoginOverlay />
      </Show>
    </>
  );
};

export default App;
