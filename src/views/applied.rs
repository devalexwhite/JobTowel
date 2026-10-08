use crate::{
    models::{
        auth::User,
        dates,
        role::{AppliedRole, Input, Role, StatusFilter},
    },
    views::layout,
};
use maud::{Markup, html};

pub fn table(
    roles: &[AppliedRole],
    csrf: &str,
    deleted: bool,
    status: StatusFilter,
    timezone: &str,
) -> Markup {
    html! {
        div id="applied-roles" {
            form action="/applied" method="get" hx-get="/applied" hx-target="#applied-roles" hx-swap="outerHTML" class="d-flex flex-wrap align-items-end gap-2 mb-3" {
                div {
                    label for="application-status-filter" class="form-label" { "Application status" }
                    select id="application-status-filter" name="status" class="form-select" {
                        @for (value, label) in [("active", "Active"), ("inactive", "Inactive"), ("all", "All applications")] {
                            option value=(value) selected[status.value() == value] { (label) }
                        }
                    }
                }
                button type="submit" class="btn btn-outline-primary" { "Apply filter" }
            }

            @if deleted { div class="alert alert-success" role="status" { "Application deleted." } }
            @if roles.is_empty() {
                p class="text-body-secondary py-4" { "No applications match this status. Change the filter or record a new application." }
            } @else {
                div class="table-responsive" {
                    table id="applications-table" data-height="800" class="table align-middle" data-applied-table data-filter-control="true" {
                        caption class="visually-hidden" { "Your recorded applications" }
                        thead { tr { th scope="col" data-field="role_title" data-sortable="true" data-filter-control="input" data-filter-control-placeholder="Filter role" { "Role" } th scope="col" data-field="company" data-sortable="true" data-filter-control="input" data-filter-control-placeholder="Filter companies" { "Company" } th scope="col" data-field="last_event_title" { "Movement" } th scope="col" data-field="status" { "Status" } th scope="col" data-field="actions" data-searchable="false" { span class="visually-hidden" { "Actions" } } } }
                        tbody {
                            @for application in roles {
                                @let role = &application.role;
                                tr {
                                    td class="text-break" { (&role.role_title) }
                                    td class="text-break" { (&role.company) }
                                    td class="text-break" {
                                        (application.last_event_title.as_deref().unwrap_or("No events"))
                                        @if let Some(date) = &application.last_event_at {
                                            time class="d-block small text-body-secondary" datetime=(date) title=(dates::display(date, timezone)) {
                                                (dates::display(date, timezone))
                                            }
                                        }
                                    }
                                    td {
                                        (if role.active { "Active" } else { "Inactive" })
                                        @if role.active {
                                            @if let Some(days) = application.active_days {
                                                span class="d-block small text-body-secondary" {
                                                    (days) " " (if days == 1 { "day" } else { "days" })
                                                }
                                            }
                                        }
                                    }
                                    td { div class="d-flex gap-2" {
                                        a class="btn btn-outline-primary btn-sm" href=(format!("/applied/{}", role.id)) aria-label=(format!("View {} at {}", role.role_title, role.company)) { "View" }
                                        a class="btn btn-outline-primary btn-sm" href=(format!("/applied/{}/edit", role.id)) aria-label=(format!("Edit {} at {}", role.role_title, role.company)) { "Edit" }
                                        a class="btn btn-outline-danger btn-sm" href=(format!("/applied/{}/delete", role.id)) hx-post=(format!("/applied/{}/delete?status={}", role.id, status.value())) hx-vals=(format!(r#"{{"csrf":"{csrf}"}}"#)) hx-confirm=(format!("Delete {} at {}? This cannot be undone.", role.role_title, role.company)) hx-target="#applied-roles" hx-swap="outerHTML" hx-disabled-elt="this" aria-label=(format!("Delete {} at {}", role.role_title, role.company)) { "Delete" }
                                    } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
pub fn index(user: &User, csrf: &str, roles: &[AppliedRole], status: StatusFilter) -> Markup {
    layout::shell(
        "Applied",
        html! {
            h1 class="h2 mb-4" { "Applied" }
            a class="btn btn-primary mb-4" href="/applied/new" { "Record new application" }
            div id="request-error" role="alert" {}
            (table(roles, csrf, false, status, user.timezone()))
        },
        Some(user),
        csrf,
    )
}
pub fn form(input: &Input, csrf: &str, id: Option<i64>, error: Option<&str>) -> Markup {
    let action = id.map_or_else(|| "/applied/new".into(), |id| format!("/applied/{id}/edit"));
    html! {
        form id="role-form" action=(action) method="post" hx-post=(action) hx-target="#role-form" hx-swap="outerHTML" hx-disabled-elt="find button" novalidate aria-describedby=[error.map(|_| "role-validation-error")] {
            input type="hidden" name="csrf" value=(csrf);
            @if let Some(error) = error { div id="role-validation-error" class="alert alert-danger" role="alert" tabindex="-1" data-validation-error { (error) } }
            @for (name, label, kind, value, max, required) in [
                ("role_title", "Role", "text", input.role_title.as_str(), "200", true),
                ("company", "Company", "text", input.company.as_str(), "200", true),
                ("posting_url", "Posting URL (optional)", "url", input.posting_url.as_str(), "2048", false),
            ] {
                div class="mb-3" {
                    label class="form-label" for=(name) { (label) }
                    input class="form-control" type=(kind) name=(name) id=(name) value=(value) maxlength=(max) required[required];
                }
            }
            div class="mb-4" {
                label class="form-label" for="role_description" { "Role Description (optional)" }
                (crate::views::notes::editor("role_description", &input.role_description, 8))
            }
            @if id.is_some() {
                input type="hidden" name="status_present" value="true";
                div class="form-check form-switch mb-4" {
                    input class="form-check-input" type="checkbox" role="switch" id="active" name="active" value="true" checked[if input.status_present == "true" { input.active.as_deref() == Some("true") } else { input.active.as_deref() != Some("false") }] aria-describedby="active-help";
                    label for="active" class="form-check-label" { "Active application" }
                    div id="active-help" class="form-text" { "Turn off to mark this application inactive." }
                }
            }
            div class="d-flex gap-3 align-items-center" {
                button class="btn btn-primary" type="submit" {
                    (if id.is_some() { "Save changes" } else { "Record application" })
                    span class="htmx-indicator spinner-border spinner-border-sm ms-2" aria-hidden="true" {}
                }
                a href="/applied" { "Cancel" }
                span class="htmx-indicator visually-hidden" role="status" { "Saving…" }
            }
        }
    }
}
pub fn form_page(user: &User, csrf: &str, form: Markup, editing: bool) -> Markup {
    let title = if editing {
        "Edit application"
    } else {
        "Record new application"
    };
    layout::shell(
        title,
        html! {
            div class="role-form-container mx-auto" {
                h1 class="h2 mb-4" { (title) }
                div id="request-error" role="alert" {}
                (form)
            }
        },
        Some(user),
        csrf,
    )
}
pub fn confirmation(user: &User, csrf: &str, role: &Role) -> Markup {
    let action = format!("/applied/{}/delete", role.id);
    layout::shell(
        "Delete application",
        html! {
            h1 class="h2" { "Delete application?" }
            p { "Delete " strong { (&role.role_title) } " at " strong { (&role.company) } "? This cannot be undone." }
            div id="request-error" role="alert" {}
            form method="post" action=(action) hx-post=(action) hx-disabled-elt="find button" {
                input type="hidden" name="csrf" value=(csrf);
                button class="btn btn-danger me-3" type="submit" { "Delete application" }
                a href="/applied" { "Cancel" }
            }
        },
        Some(user),
        csrf,
    )
}
