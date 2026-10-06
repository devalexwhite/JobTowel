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
  models/             Users, sessions, validation, parameterized SQL
  views/              Maud layout, dashboard, auth/account forms
  bin/migrate.rs      Standalone migration command
migrations/           Ordered SQL migrations
frontend/scss/        Bootstrap and application styles
frontend/js/          Bootstrap, HTMX, browser interaction
scripts/              npm asset build/watch process
tests/                HTTP integration tests against temporary SQLite files
```

## Accounts and dashboard

`/` and `/dashboard` require login. `/signup` asks for username, password, name, and email, then signs the user in. `/login` accepts username and password. Usernames are case-insensitive and use 3–32 ASCII letters, numbers, underscores, or hyphens. Passwords require 12–128 characters; no confirmation field is used. Email addresses are case-insensitive and unique; email delivery and verification are not implemented.

The dashboard has Home navigation, a welcome message, and a profile menu linking to `/account` and a CSRF-protected POST to `/logout`. Account settings allow name, email, and password changes. Only password changes require the current password; name and email changes do not. Leaving the new password blank preserves it. Password changes revoke all prior sessions. Successful authentication and account updates rotate the current session.

Passwords use Argon2id with random salts, computed outside the async runtime. Sessions use cryptographically random tokens; only token hashes are stored in SQLite. Login sessions expire after seven days, anonymous form sessions after one hour. Cookies use HttpOnly and SameSite=Lax; set `SESSION_COOKIE_SECURE=true` in HTTPS deployments. Auth forms use HTMX to render server validation as a 422 form fragment with non-password drafts preserved; successful HTMX writes use `HX-Redirect`. JavaScript-free forms use complete validation pages and 303 redirects. Password fields are never repopulated.

## SQLite migrations

```sh
cargo run --bin migrate
```

The server also runs this same migrator automatically. SQLx tracks applied versions and checksums in `_sqlx_migrations`. Applied migrations should never be edited; add a new migration instead. The retired architecture-demo migration (`20261006000000`) is the sole allowed missing version, so existing development databases can still start. Its old table and migration history are left untouched; new databases contain only the application tables. `build.rs` ensures migration changes rebuild the embedded migration set. Compilation does not need a running database: queries use typed `query_as` with integration tests verifying SQL against the real schema.

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
| `SESSION_COOKIE_SECURE` | `false` | Enable Secure session cookies; set `true` behind HTTPS |
| `RUST_LOG` | `jobtowel=info,tower_http=info` | Tracing filter |

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
npm run build
```

Tests cover authentication, protected pages, account updates, session rotation and expiry, CSRF protection, HTMX validation fragments, JavaScript-free responses, invalid input, escaping, safe database errors, and migration compatibility.

## Extending and deploying

Add domain types and SQL in `models`, templates in `views`, handlers in `controllers`, then register them in `routes`. Introduce services when business logic spans models; keep handlers small rather than adding layers without a use case.

Build assets with `npm ci && npm run build`, then compile with `cargo build --release`. Ship the executable and `static/assets`, provide a writable persistent SQLite location, and set environment variables. The binary embeds migrations but serves assets from disk. SIGTERM and Ctrl-C stop accepting requests and drain in-flight work before closing the pool.

Add rate limits as the application grows. Browser cross-origin POSTs are rejected using Origin and Fetch Metadata; origin-less API clients are allowed. Authentication forms validate session-bound CSRF tokens. Behind a reverse proxy, preserve the original Host header. Run TLS at the reverse proxy and back up SQLite using its backup API or a consistent snapshot including WAL state.

## Reference documentation

[Axum](https://docs.rs/axum/latest/axum/), [SQLx migrations](https://docs.rs/sqlx/latest/sqlx/macro.migrate.html), [Maud](https://maud.lambda.xyz/), [Bootstrap Sass](https://getbootstrap.com/docs/5.3/customize/sass/), [HTMX](https://htmx.org/docs/).
