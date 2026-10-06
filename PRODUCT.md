# Product

<!-- impeccable:product-schema 1 -->

## Platform
web

## Stack
Rust, Axum, SQLx, SQLite, Maud, Bootstrap built via npm, and HTMX, specified by the user.

## Product Purpose
Provide a server-rendered platform with user sign-up, login, a personal dashboard, and account management.

## Capabilities and Constraints
Separate models, views, controllers, and routes. Protect the dashboard and account pages with database-backed login sessions. Sign-up collects username, password without confirmation, name, and email. Account settings edit name, email, and password. Use HTMX for form validation with JavaScript-free fallbacks. Apply versioned SQLite migrations. Use conventional Bootstrap styling, confirmed by the user.

## Open Decisions
The eventual application audience, domain features, and deployment environment are unspecified.
