use crate::{
    models::comment::Comment,
    views::{comments, layout},
};
use maud::{Markup, html};

pub fn index(comments: &[Comment], body: &str, error: Option<&str>, posted: bool) -> Markup {
    layout::page(
        "Comments",
        html! {
            div class="comments-container mx-auto" {
                h1 class="mb-2" { "Comments" }
                p class="text-body-secondary mb-4" { "Share a thought and join the conversation." }
                div id="request-error" role="alert" {}
                (comments::panel(comments, body, error, posted))
            }
        },
    )
}
