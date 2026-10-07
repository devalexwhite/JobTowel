import htmx from 'htmx.org';
import 'bootstrap';
import 'bootstrap-table/src/bootstrap-table.js';
import 'bootstrap-table/src/extensions/filter-control/bootstrap-table-filter-control.js';

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
  if (['auth-form', 'role-form', 'event-form'].includes(event.detail.target.id)) {
    document.getElementById('request-error')?.replaceChildren();
    document.getElementById(event.detail.target.id)?.querySelector('[data-validation-error]')?.focus();
  }
});
document.addEventListener('htmx:responseError', event => {
  if (event.detail.xhr.status === 403 && event.detail.elt.closest('form, #applied-roles')) {
    const region = document.getElementById('request-error');
    if (region) {
      region.className = 'alert alert-danger';
      region.textContent = 'Your form expired. Reload the page and try again.';
    }
  }
});

// Bootstrap Table rebuilds tbody when sorting/filtering: reconnect HTMX actions.
function initializeAppliedTable() {
  const table = document.querySelector('[data-applied-table]');
  if (!table || $(table).data('bootstrap.table')) return;
  $(table).bootstrapTable({
    onPostBody() {
      htmx.process(table.querySelector('tbody'));
    },
    onPostHeader() {
      table.querySelectorAll('th[data-field="role_title"] .th-inner, th[data-field="company"] .th-inner').forEach(heading => {
        heading.tabIndex = 0;
        heading.setAttribute('role', 'button');
        heading.setAttribute('aria-label', `Sort by ${heading.textContent.trim()}`);
      });
      table.querySelectorAll('.filter-control input').forEach(input => {
        const label = input.closest('th').dataset.field === 'role_title' ? 'Role Title' : 'Company';
        input.setAttribute('aria-label', `Filter ${label}`);
      });
    },
  });
}
htmx.onLoad(initializeAppliedTable);
document.addEventListener('htmx:beforeSwap', event => {
  if (event.detail.target.id === 'applied-roles' && event.detail.shouldSwap) {
    const table = document.querySelector('[data-applied-table]');
    if (table && $(table).data('bootstrap.table')) $(table).bootstrapTable('destroy');
  }
});

document.addEventListener('keydown', event => {
  if (event.key !== 'Enter' && event.key !== ' ') return;
  const heading = event.target.closest('[data-applied-table] .th-inner[role="button"]');
  if (heading) {
    event.preventDefault();
    heading.click();
  }
});
