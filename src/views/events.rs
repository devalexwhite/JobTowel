use crate::{
    models::{
        auth::User,
        dates,
        event::{Event, Input, TITLE_OPTIONS},
        person::{self, Person},
        role::Role,
    },
    views::layout,
};
use maud::{Markup, html};
pub fn timeline(user: &User, csrf: &str, role: &Role, events: &[(Event, Vec<Person>)]) -> Markup {
    let now = chrono::Utc::now();
    layout::shell(
        "Application timeline",
        html! {
            a href="/applied" { "Back to Applied" }
            div class="d-flex flex-wrap justify-content-between align-items-start gap-3 mt-3 mb-4" {
                div { h1 class="h2 text-break" { (&role.role_title) } p class="text-body-secondary text-break" { (&role.company) } }
                a class="btn btn-outline-primary" href=(format!("/applied/{}/edit", role.id)) { "Edit application" }
            }
            @if !role.posting_url.is_empty() { p { a href=(&role.posting_url) { "Job posting" } } }
            @if !role.role_description.is_empty() { div class="notes-content mb-4" { (crate::views::notes::render(&role.role_description)) } }
            div class="d-flex flex-wrap gap-3 justify-content-between align-items-center mb-4" {
                h2 class="h4 mb-0" { "Timeline" }
                a class="btn btn-primary" href=(format!("/applied/{}/events/new", role.id)) { "Add event" }
            }
            @if events.is_empty() { p { "No events yet. Add an interview, call, or follow-up." } }
            ol class="event-timeline list-unstyled" {
                @for (event, people) in events {
                    li class=(if dates::local(&event.occurs_at, user.timezone()).is_some_and(|date| date > now) {
                        "position-relative pb-4 ps-4 event-future"
                    } else {
                        "position-relative pb-4 ps-4"
                    }) {
                        article {
                            time datetime=(&event.occurs_at) class="text-body-secondary" { (dates::display(&event.occurs_at, user.timezone())) }
                            h3 class="h5 mt-1 text-break" { (&event.title) }
                            @if !event.notes.is_empty() { div class="notes-content mb-3" { (crate::views::notes::render(&event.notes)) } }
                            @if !people.is_empty() {
                                ul class="person-avatars list-unstyled d-flex flex-wrap gap-2 mb-3" aria-label="People" {
                                    @for person in people { li { (person_avatar(person)) } }
                                }
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

fn person_avatar(person: &Person) -> Markup {
    let name = format!("{} {}", person.first_name, person.last_name)
        .trim()
        .to_owned();
    let initials: String = [&person.first_name, &person.last_name]
        .into_iter()
        .filter_map(|part| part.trim().chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    html! {
        details class="person-menu" {
            summary class="person-avatar" aria-label=(format!("Contact details for {name}")) {
                span aria-hidden="true" { (initials) }
            }
            div class="person-popover" {
                div class="person-card bg-body rounded-4 p-3 text-break" {
                    div class="d-flex justify-content-between align-items-start gap-3" {
                        p class="h5 mb-0" {
                            span class="d-block" { (&person.first_name) }
                            @if !person.last_name.is_empty() { span class="d-block" { (&person.last_name) } }
                        }
                        a class="small flex-shrink-0" href=(format!("/applied/{}/people/{}/edit", person.role_id, person.id)) aria-label=(format!("Update {name}")) { "Update" }
                    }
                    @if !person.job_title.is_empty() { p class="text-body-secondary mt-3 mb-0" { (&person.job_title) } }
                    @if !person.phone.is_empty() || !person.email.is_empty() {
                        div class="d-flex gap-2 mt-3" {
                            @if !person.phone.is_empty() {
                                a class="person-contact btn btn-outline-primary" href=(format!("tel:{}", person.phone)) aria-label=(format!("Call {name} at {}", person.phone)) title=(&person.phone) {
                                    svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
                                        path d="M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72c.13.96.36 1.9.69 2.79a2 2 0 0 1-.45 2.11L8.09 9.89a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45c.89.33 1.83.56 2.79.69A2 2 0 0 1 22 16.92z" {}
                                    }
                                }
                            }
                            @if !person.email.is_empty() {
                                a class="person-contact btn btn-outline-primary" href=(format!("mailto:{}", person.email)) aria-label=(format!("Email {name} at {}", person.email)) title=(&person.email) {
                                    svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
                                        rect x="3" y="5" width="18" height="14" rx="2" {}
                                        path d="m3 6 9 7 9-7" {}
                                    }
                                }
                            }
                        }
                    }
                    @if !person.notes.is_empty() { div class="notes-content small mt-3" { (crate::views::notes::render(&person.notes)) } }
                }
            }
        }
    }
}
pub fn form(
    input: &Input,
    csrf: &str,
    role: i64,
    id: Option<i64>,
    timezone: &str,
    error: Option<&str>,
) -> Markup {
    let choice = if !input.title_choice.is_empty() {
        input.title_choice.as_str()
    } else if TITLE_OPTIONS.contains(&input.title.as_str()) {
        input.title.as_str()
    } else if !input.title.is_empty() {
        "custom"
    } else {
        ""
    };
    let action = id.map_or_else(
        || format!("/applied/{role}/events/new"),
        |id| format!("/applied/{role}/events/{id}/edit"),
    );
    let offsets = dates::repeated_offsets(&input.occurs_at, timezone);
    let selected_offset = if input.offset_for.is_empty() || input.offset_for == input.occurs_at {
        input.utc_offset.as_str()
    } else {
        ""
    };
    html! {
        form id="event-form" method="post" action=(action) hx-post=(action) hx-target="#event-form" hx-swap="outerHTML" hx-disabled-elt="find button" novalidate aria-describedby=[error.map(|_| "event-validation-error")] {
            input type="hidden" name="csrf" value=(csrf);
            input type="hidden" name="timezone" value=(timezone);
            input type="hidden" name="offset_for" value=(&input.occurs_at);
            @if let Some(error) = error { div id="event-validation-error" class="alert alert-danger" role="alert" tabindex="-1" data-validation-error { (error) } }
            div class="mb-3" {
                label for="title_choice" class="form-label" { "Interview milestone" }
                select class="form-select" id="title_choice" name="title_choice" required data-event-title-choice {
                    option value="" selected[choice.is_empty()] { "Choose an interview milestone" }
                    @for title in TITLE_OPTIONS {
                        option value=(title) selected[choice == title] { (title) }
                    }
                    option value="custom" selected[choice == "custom"] { "Custom" }
                }
            }
            div class="mb-3" data-custom-event-title {
                label for="title" class="form-label" { "Custom milestone" }
                input class="form-control" id="title" name="title" value=(&input.title) maxlength="200" aria-describedby="custom-title-help";
                div id="custom-title-help" class="form-text" { "Required when Custom is selected." }
            }
            div class="mb-3" { label for="occurs_at" class="form-label" { "Date and time" } input class="form-control" type="datetime-local" id="occurs_at" name="occurs_at" value=(&input.occurs_at) required; }
            @if offsets.is_empty() {
                input type="hidden" name="utc_offset" value=(&input.utc_offset);
            } @else {
                div class="mb-3" {
                    label for="utc_offset" class="form-label" { "Which occurrence?" }
                    select id="utc_offset" name="utc_offset" class="form-select" required aria-describedby="offset-help" {
                        option value="" selected[!offsets.iter().any(|offset| offset == selected_offset)] { "Choose an occurrence" }
                        @for (index, offset) in offsets.iter().enumerate() {
                            option value=(offset) selected[selected_offset == offset] {
                                (if index == 0 { "First occurrence" } else { "Second occurrence" })
                            }
                        }
                    }
                    p id="offset-help" class="form-text" { "The clocks move back, so this local time occurs twice." }
                }
            }
            div class="mb-3" { label for="notes" class="form-label" { "Notes (optional)" } (crate::views::notes::editor("notes", &input.notes, 5)) }
            fieldset class="mb-4" aria-describedby="people-help" {
                legend class="h5" { "People (optional)" }
                p id="people-help" class="form-text" { "Choose people from this application or add someone new. Add up to 20 people per event. New people need a first name; other details are optional." }
                @for (index, person) in input.people.iter().enumerate() {
                    (person_row(index, person, &input.available_people))
                }
                @if input.people.is_empty() { p class="text-body-secondary" { "No people added." } }
                button type="submit" name="people_action" value="add" class="btn btn-outline-secondary" formnovalidate disabled[input.people.len() >= 20] { "Add person" }
            }
            @if input.status_prompt {
                fieldset class="border rounded p-3 mb-3" tabindex="-1" data-validation-error {
                    legend class="float-none w-auto px-1 fs-6" { "Mark this application inactive?" }
                    p { "Inactive applications are hidden from the default applications table and excluded from the active application count." }
                    div class="d-flex flex-wrap gap-2" {
                        button type="submit" name="removal_status" value="inactive" class="btn btn-primary" { "Add event and mark inactive" }
                        button type="submit" name="removal_status" value="keep" class="btn btn-outline-primary" { "Add event and keep current status" }
                    }
                }
            }
            div class="d-flex gap-3 align-items-center" {
                @if !input.status_prompt { button class="btn btn-primary" type="submit" { (if id.is_some() { "Save changes" } else { "Add event" }) span class="htmx-indicator spinner-border spinner-border-sm ms-2" aria-hidden="true" {} } }
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
                p { "Delete “" (&input.title) "”? Its notes and links to people will also be deleted. People linked only to this event will also be deleted. This cannot be undone." }
                form method="post" action=(format!("/applied/{}/events/{}/delete", role.id, id.unwrap())) hx-post=(format!("/applied/{}/events/{}/delete", role.id, id.unwrap())) hx-disabled-elt="find button" {
                    input type="hidden" name="csrf" value=(csrf);
                    button type="submit" class="btn btn-danger me-3" { "Delete event" }
                    a href=(format!("/applied/{}", role.id)) { "Cancel" }
                }
            } @else { (form(input, csrf, role.id, id, user.timezone(), error)) }
        } },
        Some(user),
        csrf,
    )
}

fn person_row(index: usize, person: &person::Input, available: &[Person]) -> Markup {
    html! {
        fieldset class="border rounded p-3 mb-3" {
            legend class="float-none w-auto px-1 fs-6" { "Person " (index + 1) }
            div class="mb-3" {
                @let name = format!("person_{index}_id");
                label for=(&name) class="form-label" { "Person from this application" }
                select id=(&name) name=(&name) class="form-select" data-person-choice {
                    option value="" selected[person.id.is_empty()] { "Add a new person" }
                    @if !person.id.is_empty() && !available.iter().any(|p| p.id.to_string() == person.id) {
                        option value=(&person.id) selected { "Unavailable person — choose again" }
                    }
                    @for existing in available {
                        option value=(existing.id) selected[person.id == existing.id.to_string()] {
                            (&existing.first_name) " " (&existing.last_name)
                            @if !existing.email.is_empty() { " (" (&existing.email) ")" }
                        }
                    }
                }
            }
            div class="row g-3" data-new-person {
                p class="form-text mb-0" { "Fill in these details only when adding a new person." }
                @for (field, label, value, kind, required) in [
                    ("first_name", "First name", &person.first_name, "text", true),
                    ("last_name", "Last name (optional)", &person.last_name, "text", false),
                    ("job_title", "Job title (optional)", &person.job_title, "text", false),
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
                    (crate::views::notes::editor(&name, &person.notes, 3))
                }
            }
            button type="submit" name="people_action" value=(format!("remove:{index}")) class="btn btn-outline-danger btn-sm mt-3" formnovalidate aria-label=(format!("Remove person {}", index + 1)) { "Remove person" }
        }
    }
}
