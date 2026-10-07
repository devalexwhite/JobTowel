use crate::{
    models::{
        auth::User,
        event::{Event, Input, Person},
        role::Role,
    },
    views::layout,
};
use maud::{Markup, html};
pub fn timeline(user: &User, csrf: &str, role: &Role, events: &[(Event, Vec<Person>)]) -> Markup {
    layout::shell(
        "Application timeline",
        html! {
            a href="/applied" { "Back to Applied" }
            div class="d-flex flex-wrap justify-content-between align-items-start gap-3 mt-3 mb-4" {
                div { h1 class="h2 text-break" { (&role.role_title) } p class="text-body-secondary text-break" { (&role.company) } }
                a class="btn btn-outline-primary" href=(format!("/applied/{}/edit", role.id)) { "Edit application" }
            }
            @if !role.posting_url.is_empty() { p { a href=(&role.posting_url) { "Job posting" } } }
            @if !role.role_description.is_empty() { p class="event-notes text-break mb-4" { (&role.role_description) } }
            div class="d-flex flex-wrap gap-3 justify-content-between align-items-center mb-4" {
                h2 class="h4 mb-0" { "Timeline" }
                a class="btn btn-primary" href=(format!("/applied/{}/events/new", role.id)) { "Add event" }
            }
            p class="text-body-secondary" { "Newest first. All dates and times are UTC; future appointments are included." }
            @if events.is_empty() { p { "No events yet. Add an interview, call, or follow-up." } }
            ol class="event-timeline list-unstyled" {
                @for (event, people) in events {
                    li class="position-relative pb-4 ps-4" {
                        article {
                            time datetime=(&event.occurs_at) class="text-body-secondary" { (event.occurs_at.chars().take(16).collect::<String>().replace('T', " ")) " UTC" }
                            h3 class="h5 mt-1 text-break" { (&event.title) }
                            @if !event.notes.is_empty() { p class="event-notes text-break" { (&event.notes) } }
                            @if !people.is_empty() {
                                h4 class="h6" { "People" }
                                ul class="list-unstyled" { @for person in people { li class="text-break mb-2" {
                                    strong { (&person.first_name) }
                                    @if !person.last_name.is_empty() { " " (&person.last_name) }
                                    @if !person.email.is_empty() { br; span { (&person.email) } }
                                    @if !person.phone.is_empty() { br; span { (&person.phone) } }
                                    @if !person.notes.is_empty() { p class="event-notes mb-0" { (&person.notes) } }
                                } } }
                            }
                            div class="d-flex gap-3" {
                                a href=(format!("/applied/{}/events/{}/edit", role.id, event.id)) aria-label=(format!("Edit {}", event.title)) { "Edit" }
                                a class="text-danger" href=(format!("/applied/{}/events/{}/delete", role.id, event.id)) aria-label=(format!("Delete {}", event.title)) { "Delete" }
                            }
                        }
                    }
                }
            }
        },
        Some(user),
        csrf,
    )
}
pub fn form(input: &Input, csrf: &str, role: i64, id: Option<i64>, error: Option<&str>) -> Markup {
    let action = id.map_or_else(
        || format!("/applied/{role}/events/new"),
        |id| format!("/applied/{role}/events/{id}/edit"),
    );
    html! {
        form id="event-form" method="post" action=(action) hx-post=(action) hx-target="#event-form" hx-swap="outerHTML" hx-disabled-elt="find button" novalidate aria-describedby=[error.map(|_| "event-validation-error")] {
            input type="hidden" name="csrf" value=(csrf);
            @if let Some(error) = error { div id="event-validation-error" class="alert alert-danger" role="alert" tabindex="-1" data-validation-error { (error) } }
            div class="mb-3" { label for="title" class="form-label" { "Event title" } input class="form-control" id="title" name="title" value=(&input.title) maxlength="200" required; }
            div class="mb-3" { label for="occurs_at" class="form-label" { "Date and time (UTC)" } input class="form-control" type="datetime-local" id="occurs_at" name="occurs_at" value=(&input.occurs_at) required; }
            div class="mb-3" { label for="notes" class="form-label" { "Notes (optional)" } textarea class="form-control" id="notes" name="notes" rows="5" maxlength="10000" { (&input.notes) } }
            fieldset class="mb-4" aria-describedby="people-help" {
                legend class="h5" { "People (optional)" }
                p id="people-help" class="form-text" { "Add up to 20 people. First name is required for each person; all other fields are optional." }
                @for (index, person) in input.people.iter().enumerate() {
                    (person_row(index, person))
                }
                @if input.people.is_empty() { p class="text-body-secondary" { "No people added." } }
                button type="submit" name="people_action" value="add" class="btn btn-outline-secondary" formnovalidate disabled[input.people.len() >= 20] { "Add person" }
            }
            div class="d-flex gap-3 align-items-center" {
                button class="btn btn-primary" type="submit" { (if id.is_some() { "Save changes" } else { "Add event" }) span class="htmx-indicator spinner-border spinner-border-sm ms-2" aria-hidden="true" {} }
                a href=(format!("/applied/{role}")) { "Cancel" }
                span class="htmx-indicator visually-hidden" role="status" { "Saving…" }
            }
        }
    }
}
pub fn page(
    user: &User,
    csrf: &str,
    role: &Role,
    input: &Input,
    id: Option<i64>,
    deleting: bool,
    error: Option<&str>,
) -> Markup {
    let title = if deleting {
        "Delete event?"
    } else if id.is_some() {
        "Edit event"
    } else {
        "Add event"
    };
    layout::shell(
        title,
        html! { div class="role-form-container mx-auto" {
            h1 class="h2 mb-3" { (title) } p class="text-break" { (&role.role_title) " at " (&role.company) }
            div id="request-error" role="alert" {}
            @if deleting {
                p { "Delete “" (&input.title) "”? Its notes and people will also be deleted. This cannot be undone." }
                form method="post" action=(format!("/applied/{}/events/{}/delete", role.id, id.unwrap())) hx-post=(format!("/applied/{}/events/{}/delete", role.id, id.unwrap())) hx-disabled-elt="find button" {
                    input type="hidden" name="csrf" value=(csrf);
                    button type="submit" class="btn btn-danger me-3" { "Delete event" }
                    a href=(format!("/applied/{}", role.id)) { "Cancel" }
                }
            } @else { (form(input, csrf, role.id, id, error)) }
        } },
        Some(user),
        csrf,
    )
}

fn person_row(index: usize, person: &Person) -> Markup {
    html! {
        fieldset class="border rounded p-3 mb-3" {
            legend class="float-none w-auto px-1 fs-6" { "Person " (index + 1) }
            div class="row g-3" {
                @for (field, label, value, kind, required) in [
                    ("first_name", "First name", &person.first_name, "text", true),
                    ("last_name", "Last name (optional)", &person.last_name, "text", false),
                    ("email", "Email (optional)", &person.email, "email", false),
                    ("phone", "Phone (optional)", &person.phone, "tel", false),
                ] {
                    @let name = format!("person_{index}_{field}");
                    div class="col-12 col-sm-6" {
                        label for=(&name) class="form-label" { (label) }
                        input id=(&name) name=(&name) class="form-control" type=(kind) value=(value) maxlength="200" required[required];
                    }
                }
                div class="col-12" {
                    @let name = format!("person_{index}_notes");
                    label for=(&name) class="form-label" { "Notes (optional)" }
                    textarea id=(&name) name=(&name) class="form-control" rows="2" maxlength="10000" { (&person.notes) }
                }
            }
            button type="submit" name="people_action" value=(format!("remove:{index}")) class="btn btn-outline-danger btn-sm mt-3" formnovalidate aria-label=(format!("Remove person {}", index + 1)) { "Remove person" }
        }
    }
}
