use crate::{
    models::{auth::User, dashboard::Summary, dates},
    views::layout,
};
use maud::{Markup, html};

pub fn page(user: &User, csrf: &str, summary: &Summary) -> Markup {
    let column = if summary.upcoming_event.is_some() {
        "col-12 col-lg-4"
    } else {
        "col-12 col-lg-6"
    };
    layout::shell(
        "Dashboard",
        html! {
            div class="page-heading" {
                div {
                    h1 class="h2" { "Welcome, " (&user.name) "." }
                }
                a class="btn btn-primary" href="/applied/new" { (layout::icon("plus")) "Record application" }
            }
            div class="row g-4 dashboard-summary" {
                div class=(column) {
                    section class="summary-panel h-100" aria-labelledby="applications-title" {
                        div class="summary-body" {
                            h2 id="applications-title" class="h5 card-title" { "Active applications" }
                            p id="application-count" class="fs-2 fw-semibold mb-2" { (summary.application_count) }
                            a href="/applied" { "View applications" }
                        }
                    }
                }
                div class=(column) {
                    section class="summary-panel h-100" aria-labelledby="movement-title" {
                        div class="summary-body text-break" {
                            h2 id="movement-title" class="h5 card-title" { "Latest movement" }
                            @if let Some(event) = &summary.latest_movement {
                                p class="fw-semibold mb-1" { (&event.event_title) }
                                p class="mb-1" { a href=(format!("/applied/{}", event.role_id)) { (&event.role_title) } }
                                p class="text-body-secondary mb-0" { (&event.company) }
                            } @else {
                                p class="text-body-secondary mb-0" { "No application events yet." }
                            }
                        }
                    }
                }
                @if let Some(event) = &summary.upcoming_event {
                    div class=(column) {
                        section class="summary-panel h-100" aria-labelledby="upcoming-title" {
                            div class="summary-body text-break" {
                                h2 id="upcoming-title" class="h5 card-title" { "Next upcoming event" }
                                p class="fw-semibold mb-1" { (&event.event_title) }
                                p class="mb-2" { a href=(format!("/applied/{}", event.role_id)) { (&event.role_title) } }
                                time datetime=(&event.occurs_at) title=(dates::display(&event.occurs_at, user.timezone())) class="text-body-secondary" {
                                    (dates::display(&event.occurs_at, user.timezone()))
                                }
                            }
                        }
                    }
                }
                div class="col-12" {
                    section class="milestones" aria-labelledby="milestones-title" {
                        div class="card-body" {
                            h2 id="milestones-title" class="h5 card-title" { "Milestones" }
                        }
                        ol class="list-group list-group-flush list-unstyled mb-0" {
                            @for day in &summary.milestone_days {
                                li class=(if day.events.is_empty() { "list-group-item milestone-day milestone-quiet" } else { "list-group-item milestone-day" }) {
                                    div class="row g-2" {
                                        div class="col-12 col-md-3" {
                                            h3 class="h6 mb-0" {
                                                time datetime=(day.date.to_string()) { (day.date.format("%A, %B %d, %Y")) }
                                            }
                                        }
                                        div class="col-12 col-md-9" {
                                            @if day.events.is_empty() {
                                                p class="text-body-secondary mb-0" { "No events." }
                                            } @else {
                                                ul class="list-unstyled mb-0" {
                                                    @for (index, event) in day.events.iter().enumerate() {
                                                        li class=(if index == 0 { "text-break" } else { "text-break mt-3" }) {
                                                            p class="fw-semibold mb-1" { (&event.event_title) }
                                                            p class="mb-1" {
                                                                a href=(format!("/applied/{}", event.role_id)) { (&event.role_title) }
                                                                " at " (&event.company)
                                                            }
                                                            time datetime=(&event.occurs_at) class="small text-body-secondary" {
                                                                (dates::display(&event.occurs_at, user.timezone()))
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
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
