use crate::models::auth::User;
use maud::{DOCTYPE, Markup, html};

fn initials(name: &str) -> String {
    let mut parts = name.split_whitespace();
    let Some(first) = parts.next() else {
        return String::new();
    };
    first
        .chars()
        .next()
        .into_iter()
        .chain(parts.next_back().and_then(|last| last.chars().next()))
        .flat_map(char::to_uppercase)
        .collect()
}

pub fn page(title: &str, content: Markup) -> Markup {
    shell(title, content, None, "")
}
pub fn shell(title: &str, content: Markup, user: Option<&User>, csrf: &str) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · Jobtowel" }
                link rel="stylesheet" href="/assets/app.css";
                script src="/assets/app.js" defer {}
            }
            body {
                a class="visually-hidden-focusable" href="#main" { "Skip to content" }
                header class="border-bottom bg-white" {
                    nav class="container d-flex align-items-center gap-2 gap-md-4 py-3" aria-label="Main navigation" {
                        a class="navbar-brand fw-semibold m-0" href="/" { "JobTowel" }
                        @if let Some(user) = user {
                            a class=(if title == "Dashboard" { "btn btn-primary btn-sm" } else { "btn btn-outline-primary btn-sm" }) href="/dashboard" aria-current=[(title == "Dashboard").then_some("page")] { "Home" }
                            a class=(if title == "Applied" { "btn btn-primary btn-sm" } else { "btn btn-outline-primary btn-sm" }) href="/applied" aria-current=[(title == "Applied").then_some("page")] { "Applied" }
                            details class="profile-menu ms-auto" {
                                summary class="profile-trigger" aria-label=(format!("Profile menu for {}", user.name)) {
                                    span class="fw-semibold" aria-hidden="true" { (initials(&user.name)) }
                                }
                                div class="profile-popover bg-white rounded-3 p-2" {
                                    a class="dropdown-item rounded-2" href="/account" { "Account" }
                                    form method="post" action="/logout" {
                                        input type="hidden" name="csrf" value=(csrf);
                                        button class="dropdown-item rounded-2" type="submit" { "Log Out" }
                                    }
                                }
                            }
                        }
                    }
                }
                @if let Some(user) = user && user.timezone.is_none() {
                    form id="timezone-detection" hidden method="post" action="/account/timezone" hx-post="/account/timezone" hx-trigger="timezone-detected" hx-swap="none" {
                        input type="hidden" name="csrf" value=(csrf);
                        input type="hidden" name="timezone" value="";
                    }
                }
                main id="main" class="container py-4 py-md-5" { (content) }
            }
        }
    }
}
