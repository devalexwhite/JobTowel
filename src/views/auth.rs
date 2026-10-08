use crate::{
    models::auth::{Input, User},
    views::layout,
};
use maud::{Markup, html};

fn field(
    id: &str,
    label: &str,
    kind: &str,
    value: &str,
    autocomplete: &str,
    required: bool,
) -> Markup {
    html! {
        div class="mb-3" {
            label class="form-label" for=(id) { (label) }
            input class="form-control" id=(id) name=(id) type=(kind) value=(value) autocomplete=(autocomplete) required[required]
                maxlength=(if kind == "password" { "128" } else if id == "email" { "254" } else if id == "username" { "32" } else { "100" });
        }
    }
}
pub fn form(kind: &str, input: &Input, csrf: &str, error: Option<&str>) -> Markup {
    let action = match kind {
        "signup" => "/signup",
        "account" => "/account",
        _ => "/login",
    };
    html! {
        form id="auth-form" method="post" action=(action) hx-post=(action) hx-target="#auth-form" hx-swap="outerHTML" hx-disabled-elt="find button" aria-describedby=[error.map(|_| "auth-validation-error")] novalidate {
            input type="hidden" name="csrf" value=(csrf);
            @if let Some(error) = error {
                div class="alert alert-danger" id="auth-validation-error" role="alert" tabindex="-1" data-validation-error { (error) }
                p class="small text-body-secondary" { "For your security, please re-enter any passwords." }
            }
            @if kind != "account" { (field("username", "Username", "text", &input.username, "username", true)) }
            @if kind != "login" {
                (field("name", "Name", "text", &input.name, "name", true))
                (field("email", "Email address", "email", &input.email, "email", true))
            }
            @if kind == "signup" {
                input type="hidden" name="timezone" value=(input.timezone.as_deref().unwrap_or("")) data-browser-timezone[input.timezone.as_deref().is_none_or(str::is_empty)];
            }
            @if kind == "account" {
                div class="mb-3" {
                    label class="form-label" for="timezone" { "Timezone" }
                    select id="timezone" name="timezone" class="form-select" aria-describedby="timezone-help" {
                        @if let Some(value) = &input.timezone && !value.is_empty() && value.parse::<chrono_tz::Tz>().is_err() {
                            option value=(value) selected { (value) }
                        }
                        @for zone in chrono_tz::TZ_VARIANTS {
                            option value=(zone.name()) selected[input.timezone.as_deref().unwrap_or("UTC") == zone.name()] { (zone.name().replace('_', " ")) }
                        }
                    }
                    p id="timezone-help" class="form-text" { "Used for all event and application dates. Detected from your browser by default; you can change it here." }
                }
            }
            @if kind == "account" {
                hr class="my-4";
                h2 class="h5" { "Change password" }
                p id="password-help" class="small text-body-secondary" { "Leave the new password blank to keep your current password. Use at least 8 characters for a new password." }
                (field("password", "New password", "password", "", "new-password", false))
                (field("current_password", "Current password", "password", "", "current-password", false))
            } @else {
                (field("password", "Password", "password", "", if kind == "signup" { "new-password" } else { "current-password" }, true))
                @if kind == "signup" { p class="small text-body-secondary" { "Use at least 8 characters." } }
            }
            button class="btn btn-primary w-100" type="submit" {
                (match kind { "signup" => "Create account", "account" => "Save changes", _ => "Log in" })
                span class="htmx-indicator spinner-border spinner-border-sm ms-2" aria-hidden="true" {}
            }
            span class="htmx-indicator visually-hidden" role="status" { "Submitting…" }
        }
    }
}
pub fn page(kind: &str, form: Markup, user: Option<&User>, csrf: &str, saved: bool) -> Markup {
    let title = match kind {
        "signup" => "Create your account",
        "account" => "Account",
        _ => "Welcome back",
    };
    layout::shell(
        title,
        html! {
            section class="auth-container mx-auto" aria-labelledby="page-title" {
                h1 id="page-title" class="h2 mb-2" { (title) }
                p class="text-body-secondary mb-4" {
                    (match kind { "signup" => "Sign up to get started with Jobtowel.", "account" => "Manage your personal details and password.", _ => "Log in to your Jobtowel dashboard." })
                }
                @if let Some(user) = user { p class="small text-body-secondary" { "Username: " strong { (&user.username) } } }
                @if saved { div class="alert alert-success" role="status" { "Your account has been updated." } }
                div id="request-error" role="alert" {}
                (form)
                @if kind == "login" { p class="mt-4 text-center" { "New to Jobtowel? " a href="/signup" { "Sign up" } } }
                @if kind == "signup" { p class="mt-4 text-center" { "Already have an account? " a href="/login" { "Log in" } } }
            }
        },
        user,
        csrf,
    )
}
