# Jobtowel

A Rust MVC foundation using Axum, SQLx, SQLite, Maud, Bootstrap, and HTMX.

## Run locally

Requires stable Rust (edition 2024) and Node.js 22+ with npm.

```sh
cp .env.example .env
npm ci
npm run build
cargo run
```

Open http://127.0.0.1:3000. The app creates `jobtowel.db` and applies pending migrations before accepting requests. Generated frontend assets must exist before startup.

For frontend development, run `npm run dev` in a second terminal. It builds once and watches Sass and JavaScript. Restart `cargo run` after Rust changes; optionally use `cargo watch -x run` if installed. Bootstrap CSS compiles from Sass, and esbuild bundles Bootstrap JavaScript and HTMX locally. No CDN is required. Commit both dependency lockfiles; build output and local database files are ignored.

## Architecture

```text
src/
  main.rs             Configuration, logging, startup, shutdown
  lib.rs              Library modules and shared application state
  config.rs           Environment configuration
  db.rs               SQLite pool and embedded migration runner
  error.rs            Logged internal errors with safe HTTP responses
  routes/             URL mappings, middleware, static asset serving
  controllers/        Request extraction and model/view coordination
  models/             Comment records, validation, parameterized SQL
  views/              Maud layout, index, form, and comment list
  bin/migrate.rs      Standalone migration command
migrations/           Ordered SQL migrations
frontend/scss/        Bootstrap and application styles
frontend/js/          Bootstrap, HTMX, browser interaction
scripts/              npm asset build/watch process
tests/                HTTP integration tests against temporary SQLite files
```

`GET /` calls the index controller, queries comments, and renders a complete page. `POST /comments` validates and inserts a `Comment`. With `HX-Request: true`, it renders just the comments panel, including a cleared form and every saved comment in newest-first order. Without HTMX, it uses a 303 redirect back to the index. Validation returns 422 with the draft preserved. Maud escapes all user content; queries bind parameters. The model accepts only validated input for inserts.

The HTMX target contains both the form and the list so field errors, success messages, and comments stay consistent in one swap. Only successful writes clear the draft. Database errors retarget a separate error region, preserving the form.

## SQLite migrations

```sh
cargo run --bin migrate
```

The server also runs this same migrator automatically. SQLx tracks applied versions and checksums in `_sqlx_migrations`. Applied migrations should never be edited; add a new migration instead. `build.rs` ensures migration changes rebuild the embedded migration set. Compilation does not need a running database: queries use typed `query_as` with integration tests verifying SQL against the real schema.

Add a migration manually with a unique increasing timestamp, such as `migrations/20261007000000_add_example.sql`, or optionally install SQLx CLI:

```sh
cargo install sqlx-cli --version 0.8.6 --no-default-features --features sqlite
sqlx migrate add add_example
sqlx migrate run
sqlx migrate info
```

CLI commands use `DATABASE_URL` from `.env`. This foundation uses forward-only migrations: back up the database before schema changes and fix issues with another migration. SQLite WAL, foreign keys, a bounded pool, and a busy timeout are configured centrally. Parent directories for custom database paths must already exist.

## Configuration

Copy `.env.example` to `.env`; real environment variables override `.env`.

| Variable | Default | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | `sqlite://jobtowel.db` | SQLite connection string |
| `BIND_ADDR` | `127.0.0.1:3000` | Socket address |
| `ASSET_DIR` | `static/assets` | Built asset directory, relative to working directory |
| `RUST_LOG` | `jobtowel=info,tower_http=info` | Tracing filter |

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
npm run build
```

Tests cover the page, HTMX fragments, JavaScript-free submission, SQLite persistence across reconnects, idempotent migrations, Unicode limits, invalid input, escaping, same-origin protection, request size limits, and safe database errors.

## Extending and deploying

Add domain types and SQL in `models`, templates in `views`, handlers in `controllers`, then register them in `routes`. Introduce services when business logic spans models; keep handlers small rather than adding layers without a use case.

Build assets with `npm ci && npm run build`, then compile with `cargo build --release`. Ship the executable and `static/assets`, provide a writable persistent SQLite location, and set environment variables. The binary embeds migrations but serves assets from disk. SIGTERM and Ctrl-C stop accepting requests and drain in-flight work before closing the pool.

This demo intentionally lists all comments as requested. Add pagination, authentication, moderation, and rate limits as the application grows. Browser cross-origin POSTs are rejected using Origin and Fetch Metadata; origin-less API clients are allowed. Add session-bound CSRF tokens when adding cookie authentication. Behind a reverse proxy, preserve the original Host header. Run TLS at the reverse proxy and back up SQLite using its backup API or a consistent snapshot including WAL state.

## Reference documentation

[Axum](https://docs.rs/axum/latest/axum/), [SQLx migrations](https://docs.rs/sqlx/latest/sqlx/macro.migrate.html), [Maud](https://maud.lambda.xyz/), [Bootstrap Sass](https://getbootstrap.com/docs/5.3/customize/sass/), [HTMX](https://htmx.org/docs/).
