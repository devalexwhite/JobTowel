use crate::{
    models::{auth::User, person::Input, role::Role},
    views::layout,
};
use maud::{Markup, html};

pub fn page(
    user: &User,
    csrf: &str,
    role: &Role,
    id: i64,
    input: &Input,
    error: Option<&str>,
) -> Markup {
    layout::shell(
        "Update person",
        html! {
            div class="role-form-container mx-auto" {
                h1 class="h2 mb-3" { "Update person" }
                p class="text-break" { (&role.role_title) " at " (&role.company) }
                p class="text-body-secondary" { "Changes apply to every event linked to this person in this application." }
                div id="request-error" role="alert" {}
                (form(input, csrf, role.id, id, error))
            }
        },
        Some(user),
        csrf,
    )
}
pub fn form(input: &Input, csrf: &str, role: i64, id: i64, error: Option<&str>) -> Markup {
    let action = format!("/applied/{role}/people/{id}/edit");
    html! {
        form id="person-form" method="post" action=(action) hx-post=(action) hx-target="#person-form" hx-swap="outerHTML" hx-disabled-elt="find button" novalidate aria-describedby=[error.map(|_| "person-validation-error")] {
            input type="hidden" name="csrf" value=(csrf);
            @if let Some(error) = error { div id="person-validation-error" class="alert alert-danger" role="alert" tabindex="-1" data-validation-error { (error) } }
            @for (name, label, kind, value, required) in [
                ("first_name", "First name", "text", &input.first_name, true),
                ("last_name", "Last name (optional)", "text", &input.last_name, false),
                ("job_title", "Job title (optional)", "text", &input.job_title, false),
                ("email", "Email (optional)", "email", &input.email, false),
                ("phone", "Phone (optional)", "tel", &input.phone, false),
            ] {
                div class="mb-3" {
                    label class="form-label" for=(name) { (label) }
                    input class="form-control" id=(name) name=(name) type=(kind) value=(value) maxlength="200" required[required];
                }
            }
            div class="mb-4" {
                label class="form-label" for="notes" { "Notes (optional)" }
                (crate::views::notes::editor("notes", &input.notes, 5))
            }
            div class="d-flex gap-3 align-items-center" {
                button class="btn btn-primary" type="submit" { "Save changes" span class="htmx-indicator spinner-border spinner-border-sm ms-2" aria-hidden="true" {} }
                a href=(format!("/applied/{role}")) { "Cancel" }
                span class="htmx-indicator visually-hidden" role="status" { "Saving…" }
            }
        }
    }
}
