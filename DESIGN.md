# Interface conventions

The user chose conventional Bootstrap styling. Use Bootstrap 5.3 variables and components, its system font stack, light surfaces, primary buttons, standard form labels, visible focus, and inline validation. Authentication and account pages use a centered, responsive form column capped at 28rem. The dashboard follows the supplied sketch: JobTowel wordmark at left, Home navigation beside it, circular profile control showing uppercase initials from the first and last names (one initial for a single name) at right with Account and Log Out in its dropdown. The dashboard includes a personal welcome message. Use a native details menu with keyboard focus, Escape dismissal, and click-outside dismissal. Do not add a custom brand or ornamental components to this foundation.

Server and network errors appear near the form without clearing draft text.

Auth forms use inline server validation returned by HTMX, preserve non-password drafts, and leave passwords blank after errors. Account updates show a success message after redirect.
