import htmx from 'htmx.org';
import 'bootstrap';

// HTMX keeps server-rendered validation and recoverable database errors visible.
htmx.config.responseHandling = [
  { code: '204', swap: false },
  { code: '[23]..', swap: true },
  { code: '422', swap: true, error: false },
  { code: '500', swap: true, error: true },
  { code: '[45]..', swap: false, error: true },
];
htmx.config.selfRequestsOnly = true;
htmx.config.allowEval = false;
htmx.config.allowScriptTags = false;

// The server replaces the form, resetting it only after a successful write.
document.addEventListener('htmx:afterSwap', event => {
  if (event.detail.target.id === 'comments-panel') {
    const message = document.getElementById('request-error');
    if (message) { message.replaceChildren(); message.className = ''; }
    document.getElementById('comment-body')?.focus({ preventScroll: true });
  }
});
document.addEventListener('htmx:sendError', () => {
  const message = document.getElementById('request-error');
  if (message) {
    message.className = 'alert alert-danger';
    message.textContent = 'Connection lost. Check your connection and try posting again.';
  }
});
