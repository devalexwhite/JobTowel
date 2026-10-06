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

document.addEventListener('htmx:sendError', () => {
  const message = document.getElementById('request-error');
  if (message) {
    message.className = 'alert alert-danger';
    message.textContent = 'Connection lost. Check your connection and try again.';
  }
});

// Native details keeps the profile menu usable without JavaScript.
document.addEventListener('click', event => {
  document.querySelectorAll('.profile-menu[open]').forEach(menu => {
    if (!menu.contains(event.target)) menu.removeAttribute('open');
  });
});
document.addEventListener('keydown', event => {
  if (event.key === 'Escape') {
    document.querySelectorAll('.profile-menu[open]').forEach(menu => {
      menu.removeAttribute('open');
      menu.querySelector('summary').focus();
    });
  }
});
document.addEventListener('htmx:afterSwap', event => {
  if (event.detail.target.id === 'auth-form') {
    document.getElementById('request-error')?.replaceChildren();
    document.querySelector('#auth-form [data-validation-error]')?.focus();
  }
});
document.addEventListener('htmx:responseError', event => {
  if (event.detail.xhr.status === 403 && event.detail.elt.closest('#auth-form')) {
    const region = document.getElementById('request-error');
    if (region) {
      region.className = 'alert alert-danger';
      region.textContent = 'Your form expired. Reload the page and try again.';
    }
  }
});
