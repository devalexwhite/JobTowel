# Product

<!-- impeccable:product-schema 1 -->

## Platform
web

## Stack
Rust, Axum, SQLx, SQLite, Maud, Bootstrap built via npm, and HTMX, specified by the user.

## Product Purpose
A tool for job seekers to track jobs applied to, interview stages, interviewers, notes and follow-up actions (such as sending a thank you email).

## Capabilities and Constraints
Separate models, views, controllers, and routes. Protect the dashboard and account pages with database-backed login sessions. Sign-up collects username, password without confirmation, name, and email. Account settings edit name, email, and password. Use HTMX for form validation with JavaScript-free fallbacks. Apply versioned SQLite migrations. Use conventional Bootstrap styling, confirmed by the user.

## Application Tracking
Authenticated users record and edit roles through Applied. Each role belongs to one user and stores a title, company, optional posting URL and description, and record creation/update timestamps. Only its owner can read or modify it. Deletion requires confirmation.

## Open Decisions
The domain features, and deployment environment are unspecified.

## Application events
Each application has an owner-protected event timeline. Creation records an Applied event automatically. Users create, edit, and delete dated events, notes, and associated people. People use structured, removable rows with required first name and optional last name, email, phone, and notes. Future appointments and completed interviews share the same timeline; dates and times use UTC.
