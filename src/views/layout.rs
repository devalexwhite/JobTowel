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
            body class=(if user.is_some() { "workspace" } else { "auth-page" }) {
                a class="skip-link visually-hidden-focusable" href="#main" { "Skip to content" }
                header class="app-navigation" {
                    nav class="workspace-nav" aria-label="Main navigation" {
                        a class="navbar-brand brand" href="/" {
                            (icon("brand")) "JobTowel"
                        }
                        @if let Some(user) = user {
                            div class="workspace-links" {
                                a class="workspace-link" href="/dashboard" aria-current=[(title == "Dashboard").then_some("page")] { (icon("home")) "Home" }
                                a class="workspace-link" href="/applied" aria-current=[(title == "Applied" || title.to_lowercase().contains("application") || title.contains("event") || title == "Update person").then_some("page")] { (icon("applications")) "Applied" }
                            }
                            a class="workspace-add" href="/applied/new" { (icon("plus")) "Record application" }
                            div class="workspace-profile" {
                                details class="profile-menu" {
                                    summary class="profile-trigger" aria-label=(format!("Profile menu for {}", user.name)) {
                                        span class="fw-semibold" aria-hidden="true" { (initials(&user.name)) }
                                    }
                                    div class="profile-popover rounded-3 p-2" {
                                        a class="dropdown-item rounded-2" href="/account" { "Account" }
                                        form method="post" action="/logout" {
                                            input type="hidden" name="csrf" value=(csrf);
                                            button class="dropdown-item rounded-2" type="submit" { "Log Out" }
                                        }
                                    }
                                }
                                div class="profile-name" { span { (&user.name) } a href="/account" { "Account settings" } }
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
                main id="main" class="main-content" tabindex="-1" { (content) }
            }
        }
    }
}

// A small, consistent stroke icon vocabulary, rendered locally with the page.
pub fn icon(name: &str) -> Markup {
    html! {
        svg class="ui-icon" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
            @match name {
                "home" => { path d="m3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1Z" {} }
                "applications" => { rect x="3" y="6" width="18" height="15" rx="2" {} path d="M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2M3 12h18M10 12v3h4v-3" {} }
                "plus" => { path d="M12 5v14M5 12h14" {} }
                _ => { rect x="3" y="3" width="18" height="18" rx="5" {} path d="m7 9 3 3 7-5M7 16h10" {} }
            }
        }
    }
}
