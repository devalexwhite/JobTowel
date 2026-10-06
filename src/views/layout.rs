use maud::{DOCTYPE, Markup, html};

pub fn page(title: &str, content: Markup) -> Markup {
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
                    nav class="container py-3" aria-label="Main navigation" {
                        a class="navbar-brand fw-semibold" href="/" { "Jobtowel" }
                    }
                }
                main id="main" class="container py-4 py-md-5" { (content) }
                footer class="container pb-4 small text-body-secondary" { "A place to share your thoughts." }
            }
        }
    }
}
