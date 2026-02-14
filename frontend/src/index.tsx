import { render } from 'solid-js/web';
import App from './App';
import { initStore } from './lib/store';
import './global.css';

// Initialize reactive store and WebSocket connection
initStore();

const root = document.getElementById('root');
if (root) {
  render(() => <App />, root);
}
