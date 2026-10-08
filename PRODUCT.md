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
Each application has an owner-protected event timeline. Creation records an Applied event automatically. Users create, edit, and delete dated events, notes, and associated people. People belong to an application and can be reused across its events. Users can update all six contact fields through the contact card’s Update link; changes apply across every event linked to that person. Event forms let users choose an existing application person or create a new one; removing their last event link or deleting their last linked event deletes the person record. People use structured, removable rows with required first name and optional last name, job title, email, phone, and notes. Future appointments and completed interviews share the same timeline; dates and times use the profile timezone.

Applications have an active status, enabled by default for new and existing records. Users can toggle status when editing applications, filter the application list by status (active by default), and see the active count on Home. Removed from consideration is an event preset; its creation asks whether to mark the application inactive or keep the current status.

Users have an editable profile timezone, defaulted from their browser at signup or the next visit for existing accounts. A saved timezone remains stable across travel. UTC is the no-JavaScript fallback. The profile timezone governs event entry and all application/event date displays, while stored timestamps remain UTC.

All displayed dates and times use “Month DD, YYYY at H:MM AM/PM” in the profile timezone. The timezone is visible and editable only in Account; signup detects it in the background.
