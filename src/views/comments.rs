use crate::models::comment::{Comment, MAX_COMMENT_LENGTH};
use maud::{Markup, html};

pub fn panel(comments: &[Comment], body: &str, error: Option<&str>, posted: bool) -> Markup {
    html! {
        section id="comments-panel" aria-label="Comment form and conversation" {
            form id="comment-form" action="/comments" method="post"
                hx-post="/comments" hx-target="#comments-panel" hx-swap="outerHTML"
                hx-disabled-elt="find button" hx-indicator="#posting-indicator" {
                div class="mb-3" {
                    label for="comment-body" class="form-label fw-semibold" { "Your comment" }
                    textarea id="comment-body" name="body" rows="4" required
                        maxlength=(MAX_COMMENT_LENGTH)
                        class=(if error.is_some() { "form-control is-invalid" } else { "form-control" })
                        aria-invalid=(if error.is_some() { "true" } else { "false" })
                        aria-describedby="comment-help comment-error" { (body) }
                    div id="comment-help" class="form-text" { "Up to 2,000 characters. All comments are public." }
                    div id="comment-error" class="invalid-feedback" role="alert" {
                        @if let Some(message) = error { (message) }
                    }
                }
                div class="d-flex align-items-center gap-3" {
                    button type="submit" class="btn btn-primary" { "Post comment" }
                    span id="posting-indicator" class="htmx-indicator small text-body-secondary" role="status" { "Posting…" }
                }
            }
            div class="mt-3" role="status" aria-live="polite" {
                @if posted { p class="text-success mb-0" { "Your comment was posted." } }
            }
            section class="mt-5" aria-labelledby="comments-heading" {
                h2 id="comments-heading" class="h4 mb-3" {
                    "All comments " span class="text-body-secondary fw-normal" { "(" (comments.len()) ")" }
                }
                (list(comments))
            }
        }
    }
}

pub fn list(comments: &[Comment]) -> Markup {
    html! {
        div id="comments-list" {
            @if comments.is_empty() {
                p class="border rounded p-4 text-body-secondary bg-white" { "No comments yet. Be the first to share a thought." }
            } @else {
                ol class="list-group list-group-flush" {
                    @for comment in comments {
                        li class="list-group-item px-0 py-3 bg-transparent" {
                            article aria-label=(format!("Comment {}", comment.id)) {
                                p class="comment-body mb-2" { (&comment.body) }
                                time class="small text-body-secondary" datetime=(&comment.created_at) {
                                    (&comment.created_at.replace('T', " ").trim_end_matches('Z')) " UTC"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
