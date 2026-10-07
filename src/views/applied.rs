use crate::{
    models::{
        auth::User,
        role::{AppliedRole, Input, Role},
    },
    views::layout,
};
use maud::{Markup, html};

pub fn table(roles: &[AppliedRole], csrf: &str, deleted: bool) -> Markup {
    html! {
        div id="applied-roles" {
            @if deleted { div class="alert alert-success" role="status" { "Application deleted." } }
            @if roles.is_empty() {
                p class="text-body-secondary py-4" { "No applications recorded yet. Record your first application to get started." }
            } @else {
                div class="table-responsive" {
                    table id="applications-table" data-height="800" class="table align-middle" data-applied-table data-filter-control="true" {
                        caption class="visually-hidden" { "Your recorded applications" }
                        thead { tr { th scope="col" data-field="role_title" data-sortable="true" data-filter-control="input" data-filter-control-placeholder="Filter role titles" { "Role Title" } th scope="col" data-field="company" data-sortable="true" data-filter-control="input" data-filter-control-placeholder="Filter companies" { "Company" } th scope="col" data-field="last_event_title" { "Last Event" } th scope="col" data-field="actions" data-searchable="false" { span class="visually-hidden" { "Actions" } } } }
                        tbody {
                            @for application in roles {
                                @let role = &application.role;
                                tr {
                                    td class="text-break" { (&role.role_title) }
                                    td class="text-break" { (&role.company) }
                                    td class="text-break" {
                                        (application.last_event_title.as_deref().unwrap_or("No events"))
                                        @if let (Some(date), Some(days)) = (&application.last_event_at, application.last_event_days_ago) {
                                            time class="d-block small text-body-secondary" datetime=(date) title=(format!("{} UTC", date.replace('T', " ").trim_end_matches('Z'))) {
                                                (relative_event_date(days))
                                            }
                                        }
                                    }
                                    td { div class="d-flex gap-2" {
                                        a class="btn btn-outline-primary btn-sm" href=(format!("/applied/{}", role.id)) aria-label=(format!("View {} at {}", role.role_title, role.company)) { "View" }
                                        a class="btn btn-outline-primary btn-sm" href=(format!("/applied/{}/edit", role.id)) aria-label=(format!("Edit {} at {}", role.role_title, role.company)) { "Edit" }
                                        a class="btn btn-outline-danger btn-sm" href=(format!("/applied/{}/delete", role.id)) hx-post=(format!("/applied/{}/delete", role.id)) hx-vals=(format!(r#"{{"csrf":"{csrf}"}}"#)) hx-confirm=(format!("Delete {} at {}? This cannot be undone.", role.role_title, role.company)) hx-target="#applied-roles" hx-swap="outerHTML" hx-disabled-elt="this" aria-label=(format!("Delete {} at {}", role.role_title, role.company)) { "Delete" }
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
fn relative_event_date(days_ago: i64) -> String {
    match days_ago {
        0 => return "Today".into(),
        1 => return "Yesterday".into(),
        -1 => return "Tomorrow".into(),
        _ => {}
    }
    let days = days_ago.unsigned_abs();
    let (count, unit) = match days {
        2..=6 => (days, "day"),
        7..=29 => (days / 7, "week"),
        30..=364 => (days / 30, "month"),
        _ => (days / 365, "year"),
    };
    if days_ago < 0 {
        if count == 1 {
            format!("In a {unit}")
        } else {
            format!("In {count} {unit}s")
        }
    } else if count == 1 {
        format!("A {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
}

pub fn index(user: &User, csrf: &str, roles: &[AppliedRole]) -> Markup {
    layout::shell(
        "Applied",
        html! {
            h1 class="h2 mb-4" { "Applied" }
            a class="btn btn-primary mb-4" href="/applied/new" { "Record new application" }
            div id="request-error" role="alert" {}
            (table(roles, csrf, false))
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
                ("role_title", "Role Title", "text", input.role_title.as_str(), "200", true),
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
                textarea class="form-control" id="role_description" name="role_description" rows="8" maxlength="10000" { (&input.role_description) }
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

#[cfg(test)]
mod tests {
    use super::relative_event_date;

    #[test]
    fn relative_dates_cover_past_future_and_unit_boundaries() {
        for (days, expected) in [
            (0, "Today"),
            (1, "Yesterday"),
            (2, "2 days ago"),
            (6, "6 days ago"),
            (7, "A week ago"),
            (14, "2 weeks ago"),
            (29, "4 weeks ago"),
            (30, "A month ago"),
            (60, "2 months ago"),
            (364, "12 months ago"),
            (365, "A year ago"),
            (730, "2 years ago"),
            (-1, "Tomorrow"),
            (-2, "In 2 days"),
            (-7, "In a week"),
            (-30, "In a month"),
            (-365, "In a year"),
        ] {
            assert_eq!(relative_event_date(days), expected);
        }
    }
}
