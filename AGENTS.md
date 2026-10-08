# Project guidance

Jobtowel is a server-rendered Rust MVC application. Preserve the existing stack and separation of responsibilities when extending it. See `README.md` for setup and deployment, `PRODUCT.md` for scope, and `DESIGN.md` for interface conventions.

## Technology stack

- Rust edition 2024 with Tokio for the async runtime.
- Axum 0.8 for HTTP routing, extractors, and responses; tower-http for middleware and static files.
- SQLx 0.8 with SQLite for persistence and embedded migrations.
- Maud 0.27 for typed, server-rendered HTML and partial views.
- Bootstrap 5.3 for conventional, responsive styling; HTMX 2 for partial HTML updates.
- npm with Node.js 22+; Sass compiles Bootstrap CSS and esbuild bundles browser JavaScript.
- Serde for form deserialization, thiserror for application errors, and tracing for logs.

Dependency manifests and lockfiles are authoritative for exact versions. Do not introduce a SPA framework, alternate ORM, template engine, database, or CSS framework unless the task calls for a stack change.

## Architecture and ownership

| Location | Responsibility |
| --- | --- |
| `src/main.rs` | Configure logging, initialize the database, apply migrations, start the server, and handle graceful shutdown. |
| `src/lib.rs` | Export modules and define cheaply cloned shared `AppState`. |
| `src/config.rs` | Load environment configuration and optional `.env` values. |
| `src/db.rs` | Configure the SQLite pool and expose the shared migration runner. |
| `src/routes/` | Map URLs to controllers; configure middleware, body limits, fallback responses, and static files. |
| `src/controllers/` | Extract requests, coordinate model operations and views, and choose HTTP responses. |
| `src/models/` | Define domain records, validate input, and execute parameterized SQL. |
| `src/views/` | Render Maud layouts, full pages, forms, and reusable HTML fragments. |
| `src/error.rs` | Log internal failures and return safe user-facing errors. |
| `src/bin/migrate.rs` | Apply migrations without starting the HTTP server. |
| `migrations/` | Store ordered, forward-only SQLite schema migrations. |
| `frontend/scss/` | Bootstrap Sass imports and application styles. |
| `frontend/js/` | Bootstrap, HTMX configuration, and browser interaction. |
| `scripts/build-assets.mjs` | Build and watch frontend assets. |
| `static/assets/` | Generated assets served under `/assets`; keep source changes in `frontend/`. |
| `tests/` | HTTP integration tests backed by isolated temporary SQLite databases. |

Keep SQL out of controllers and views, HTTP response decisions out of models, and persistence out of views. Keep controllers small. Introduce a service layer when business logic spans models rather than adding one for simple CRUD.

## HTTP and view conventions

- Use Axum `State<AppState>` for shared resources and typed extractors for request input. Body-consuming extractors such as `Form` belong last in handler arguments.
- Render complete pages through `views::layout::page`; reuse view functions for fragments.
- Support normal browser forms alongside HTMX. Successful non-HTMX POSTs use post/redirect/get with a 303 redirect.
- Authentication and account forms use `#auth-form` with HTMX `outerHTML` replacement for validation errors and `HX-Redirect` after successful writes.
- Validate input on the server before writing. Preserve non-password drafts and return 422 for validation errors. Never repopulate password fields.
- Keep HTMX response handling in `frontend/js/app.js` consistent with server status codes and fragment targets. Internal failures use the separate `#request-error` region so a draft survives.
- Use Maud's escaped interpolation for user content. Never use `PreEscaped` for untrusted input.
- Follow conventional Bootstrap styling: responsive layouts, visible labels and keyboard focus, inline errors, and accessible loading, empty, and success states.
- Serve Bootstrap and HTMX from the local npm build. Avoid CDN dependencies and inline script handlers.
- Interactive experiences should use HTMX with no-js fallbacks.

## Date and timezone conventions

- Store timestamps in UTC using the existing RFC 3339 millisecond format. Use the signed-in user’s IANA profile timezone for event input, defaults, display, and relative calendar dates.
- Share conversions through `src/models/dates.rs` using Chrono and chrono-tz; do not calculate local calendar days with SQLite UTC date expressions or fixed offsets.
- A NULL profile timezone means browser detection is pending; use UTC as the fallback. Browser detection writes through the authenticated, CSRF-protected `/account/timezone` POST and only updates an unset timezone. Explicit profile choices must win over later detection.
- Preserve date drafts and the chosen occurrence of repeated times. Reject nonexistent local times and require an occurrence choice for ambiguous times. Preserve stored instants when profile timezones change.

## Database and migration conventions

- Use the shared SQLx pool; do not create a connection pool per request. Preserve WAL mode, foreign keys, busy timeout, and bounded connections.
- Bind SQL parameters. Use typed `query_as` and `FromRow` for records; the current queries compile without a live database. Validate SQL through integration tests against migrations.
- Accept validated domain input for writes. Use transactions when multiple dependent writes must succeed together.
- Add schema changes as uniquely named, increasing timestamp migrations: `migrations/<timestamp>_<description>.sql`.
- Never edit an applied migration. Use a new forward migration for corrections. The explicitly retired architecture-demo version `20261006000000` is allowed to be absent; preserve checks for every other missing version and for active migration checksums.
- Migrations are embedded by `sqlx::migrate!`; `build.rs` watches the migration directory. Both server startup and the migration binary use `db::migrate`.
- Tests must use temporary databases, never the developer's `jobtowel.db` or another persistent database.

## Development commands

Initial setup:

```sh
cp .env.example .env
npm ci
npm run build
cargo run
```

Preserve an existing `.env`; copy the example only when the file is absent. The default URL is `http://127.0.0.1:3000`. Frontend assets must exist before server startup.

- `npm run dev`: build and watch frontend assets in a separate terminal.
- `npm run build`: generate production CSS and JavaScript.
- `cargo run`: run the application; restart after Rust changes.
- `cargo run --bin migrate`: apply pending migrations without serving HTTP.
- `cargo build --release`: compile the deployment executable; ship `static/assets` alongside it.
- `docker build -t jobtowel .`: build the deployment image, including frontend assets and both Rust binaries. See README for local container and Render deployment instructions. `render.yaml` configures one Docker web service with a persistent SQLite disk at `/var/data`; migrations run at startup, not in a pre-deploy command. The container entrypoint derives `BIND_ADDR` from Render's `PORT` unless explicitly overridden.

Configuration uses `DATABASE_URL`, `BIND_ADDR`, `ASSET_DIR`, `RUST_LOG`, and `SESSION_COOKIE_SECURE`. Keep example configuration in `.env.example`; do not commit secrets, `.env`, database files, `node_modules`, `target`, or generated assets. Commit `Cargo.lock` and `package-lock.json` when dependencies change.

## Verification

For Rust behavior changes, run the relevant integration tests and required Rust checks:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For frontend changes, run `npm run build`. For form or HTMX changes, verify actual browser submission, fragment replacement without navigation, validation and draft preservation, and responsive behavior. HTTP tests alone do not execute JavaScript.

Extend integration coverage for meaningful behavior changes, including persistence, migration compatibility, response status and HTML shape, validation, escaping, and error paths. Documentation-only changes need consistency and diff checks rather than application test runs.

## Security and operational boundaries

- Log internal errors with tracing and keep database details out of HTTP responses. Avoid logging passwords, session tokens, or form contents.
- Preserve request body limits, same-origin POST protection, and response security headers. Authentication forms use session-bound CSRF tokens, hashed database-backed session tokens, and Argon2id passwords. Preserve session rotation and password-change revocation. Set `SESSION_COOKIE_SECURE=true` for HTTPS deployments.
- Preserve JavaScript-free form submission and graceful shutdown for Ctrl-C and SIGTERM.
- Rate limiting remains a future application requirement, not an existing capability.
- Update the README and this guidance when architectural boundaries, configuration, or development commands change.
