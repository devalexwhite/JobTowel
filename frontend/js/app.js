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

function updateCustomEventTitle(form) {
  const choice = form.querySelector('[data-event-title-choice]');
  const custom = form.querySelector('[data-custom-event-title]');
  if (!choice || !custom) return;
  const selected = choice.value === 'custom';
  custom.hidden = !selected;
  custom.querySelector('input').required = selected;
}
function updatePersonFields(form) {
  form.querySelectorAll('[data-person-choice]').forEach(choice => {
    const fields = choice.closest('fieldset').querySelector('[data-new-person]');
    fields.hidden = choice.value !== '';
    fields.querySelectorAll('input, textarea, button').forEach(input => {
      input.disabled = fields.hidden;
      if (input.name.endsWith('_first_name')) input.required = !fields.hidden;
    });
  });
}
htmx.onLoad(() => {
  const form = document.getElementById('event-form');
  if (form) updatePersonFields(form);
});

// Keep the native textarea as the form value through every HTMX replacement.
htmx.onLoad(() => {
  document.querySelectorAll('[data-markdown-editor] .markdown-toolbar').forEach(toolbar => {
    toolbar.hidden = false;
  });
});
document.addEventListener('click', async event => {
  const button = event.target.closest('[data-markdown-command], [data-markdown-preview]');
  if (!button || button.disabled) return;
  const editor = button.closest('[data-markdown-editor]');
  const textarea = editor.querySelector('textarea');
  const preview = editor.querySelector('.markdown-preview');
  const previewButton = editor.querySelector('[data-markdown-preview]');
  if (button.matches('[data-markdown-preview]')) {
    if (!preview.hidden) {
      preview.hidden = true;
      button.setAttribute('aria-expanded', 'false');
      button.textContent = 'Preview';
      return;
    }
    preview.hidden = false;
    preview.textContent = 'Loading preview…';
    button.setAttribute('aria-expanded', 'true');
    button.textContent = 'Hide preview';
    button.disabled = true;
    try {
      await htmx.ajax('POST', '/notes/preview', {
        source: button, target: preview, swap: 'innerHTML',
        values: { csrf: textarea.form.elements.csrf.value, notes: textarea.value },
      });
    } catch {
      preview.textContent = 'Preview unavailable. Try again; your notes are still in the editor.';
    } finally {
      button.disabled = textarea.disabled;
    }
    return;
  }
  const start = textarea.selectionStart;
  const end = textarea.selectionEnd;
  const selected = textarea.value.slice(start, end);
  const command = button.dataset.markdownCommand;
  let prefix, suffix, content;
  if (['heading', 'list', 'quote'].includes(command)) {
    const marker = { heading: '## ', list: '- ', quote: '> ' }[command];
    prefix = start > 0 && textarea.value[start - 1] !== '\n' ? '\n' : '';
    suffix = end < textarea.value.length && textarea.value[end] !== '\n' ? '\n' : '';
    content = (selected || { heading: 'Heading', list: 'List item', quote: 'Quote' }[command])
      .split('\n').map(line => marker + line).join('\n');
  } else {
    [prefix, suffix, content] = {
      bold: ['**', '**', selected || 'bold text'],
      italic: ['*', '*', selected || 'italic text'],
      link: ['[', '](https://example.com)', selected || 'link text'],
      code: ['`', '`', selected || 'code'],
    }[command];
  }
  if (textarea.value.length - (end - start) + prefix.length + content.length + suffix.length > textarea.maxLength) return;
  textarea.focus();
  textarea.setRangeText(prefix + content + suffix, start, end, 'select');
  textarea.setSelectionRange(start + prefix.length, start + prefix.length + content.length);
  textarea.dispatchEvent(new Event('input', { bubbles: true }));
  preview.hidden = true;
  previewButton.setAttribute('aria-expanded', 'false');
  previewButton.textContent = 'Preview';
});
document.addEventListener('input', event => {
  const editor = event.target.closest('[data-markdown-editor]');
  if (!editor) return;
  editor.querySelector('.markdown-preview').hidden = true;
  const button = editor.querySelector('[data-markdown-preview]');
  button.setAttribute('aria-expanded', 'false');
  button.textContent = 'Preview';
});
document.addEventListener('change', event => {
  if (event.target.matches('[data-person-choice]')) updatePersonFields(event.target.form);
});

htmx.onLoad(() => {
  const form = document.getElementById('event-form');
  if (form) updateCustomEventTitle(form);
});
document.addEventListener('change', event => {
  if (event.target.matches('[data-event-title-choice]')) {
    updateCustomEventTitle(event.target.form);
  }
});

document.addEventListener('htmx:sendError', () => {
  const message = document.getElementById('request-error');
  if (message) {
    message.className = 'alert alert-danger';
    message.textContent = 'Connection lost. Check your connection and try again.';
  }
});

// Native details keeps contact cards usable by touch and without JavaScript.
function positionPersonCard(menu) {
  menu.classList.remove('person-menu-below');
  menu.style.removeProperty('--person-card-offset');
  const card = menu.querySelector('.person-popover');
  const bounds = card.getBoundingClientRect();
  const offset = Math.max(8 - bounds.left, Math.min(0, innerWidth - 8 - bounds.right));
  menu.style.setProperty('--person-card-offset', `${offset}px`);
  if (bounds.top < 8) menu.classList.add('person-menu-below');
}
htmx.onLoad(() => {
  document.querySelectorAll('.person-menu').forEach(menu => {
    if (menu.dataset.initialized) return;
    menu.dataset.initialized = 'true';
    let closeTimer;
    const open = () => {
      clearTimeout(closeTimer);
      document.querySelectorAll('.person-menu[open]').forEach(other => {
        if (other !== menu) other.open = false;
      });
      menu.open = true;
      positionPersonCard(menu);
    };
    const closeAfterLeave = () => {
      clearTimeout(closeTimer);
      closeTimer = setTimeout(() => {
        if (!menu.matches(':hover') && !menu.contains(document.activeElement)) menu.open = false;
      }, 150);
    };
    menu.addEventListener('pointerenter', event => {
      if (event.pointerType === 'mouse') open();
    });
    menu.addEventListener('pointerleave', closeAfterLeave);
    menu.addEventListener('focusin', event => {
      if (event.target.matches('summary:focus-visible')) open();
    });
    menu.addEventListener('focusout', closeAfterLeave);
    menu.addEventListener('toggle', () => {
      if (menu.open) positionPersonCard(menu);
    });
  });
});
window.addEventListener('resize', () => {
  document.querySelectorAll('.person-menu[open]').forEach(positionPersonCard);
});

// Dismiss the profile menu and contact cards on outside click or Escape.
document.addEventListener('click', event => {
  document.querySelectorAll('.profile-menu[open], .person-menu[open]').forEach(menu => {
    if (!menu.contains(event.target)) menu.removeAttribute('open');
  });
});
document.addEventListener('keydown', event => {
  if (event.key === 'Escape') {
    document.querySelectorAll('.profile-menu[open], .person-menu[open]').forEach(menu => {
      menu.querySelector('summary').focus();
      menu.removeAttribute('open');
    });
  }
});
document.addEventListener('htmx:afterSwap', event => {
  if (['auth-form', 'role-form', 'event-form', 'person-form'].includes(event.detail.target.id)) {
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
        const label = input.closest('th').dataset.field === 'role_title' ? 'Role' : 'Company';
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

// Only initialize an unset profile. Explicit profile choices always win.
let hasDraftChanges = false;
document.addEventListener('input', () => { hasDraftChanges = true; });
document.addEventListener('change', () => { hasDraftChanges = true; });
htmx.onLoad(() => {
  let timezone;
  try { timezone = Intl.DateTimeFormat().resolvedOptions().timeZone; } catch { return; }
  if (!timezone) return;
  const picker = document.querySelector('[data-browser-timezone]');
  if (picker && !picker.value && (picker.type === 'hidden' || Array.from(picker.options).some(option => option.value === timezone))) {
    picker.value = timezone;
    picker.removeAttribute('data-browser-timezone');
  }
  const detection = document.getElementById('timezone-detection');
  if (detection && !detection.dataset.started) {
    detection.dataset.started = 'true';
    detection.elements.timezone.value = timezone;
    htmx.trigger(detection, 'timezone-detected');
  }
});
document.addEventListener('htmx:afterRequest', event => {
  if (event.detail.elt.id === 'timezone-detection' && event.detail.successful
      && event.detail.xhr.getResponseHeader('X-Timezone-Initialized') === 'true' && !hasDraftChanges) {
    location.reload();
  }
});
