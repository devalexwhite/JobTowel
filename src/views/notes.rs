use maud::{Markup, PreEscaped, html};
use pulldown_cmark::{Options, Parser};

/// Only sanitized Markdown output may cross the escaped Maud boundary.
pub fn render(source: &str) -> Markup {
    let parser = Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    );
    let mut output = String::new();
    pulldown_cmark::html::push_html(&mut output, parser);
    // Limit HTML to note content: no images, embeds, styling, IDs or app behavior.
    let safe = ammonia::Builder::default()
        .tags(
            [
                "p",
                "br",
                "h1",
                "h2",
                "h3",
                "h4",
                "h5",
                "h6",
                "strong",
                "em",
                "del",
                "blockquote",
                "ul",
                "ol",
                "li",
                "pre",
                "code",
                "a",
                "hr",
                "table",
                "thead",
                "tbody",
                "tr",
                "th",
                "td",
            ]
            .into_iter()
            .collect(),
        )
        .tag_attributes(
            [
                ("a", ["href", "title"].into_iter().collect()),
                ("ol", ["start"].into_iter().collect()),
            ]
            .into_iter()
            .collect(),
        )
        .generic_attributes(Default::default())
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .clean(&output)
        .to_string();
    PreEscaped(safe)
}

pub fn editor(name: &str, value: &str, rows: u8) -> Markup {
    let help = format!("{name}-help");
    let preview = format!("{name}-preview");
    html! {
        div class="markdown-editor" data-markdown-editor {
            div class="markdown-toolbar d-flex flex-wrap gap-1 mb-2" role="group" aria-label="Markdown formatting" hidden {
                @for (command, label) in [("bold", "Bold"), ("italic", "Italic"), ("heading", "Heading"), ("list", "List"), ("quote", "Quote"), ("link", "Link"), ("code", "Code")] {
                    button type="button" class="btn btn-outline-secondary btn-sm" data-markdown-command=(command) aria-controls=(name) { (label) }
                }
                button type="button" class="btn btn-outline-primary btn-sm" data-markdown-preview aria-controls=(&preview) aria-expanded="false" { "Preview" }
            }
            textarea class="form-control" id=(name) name=(name) rows=(rows) maxlength="10000" aria-describedby=(&help) { (value) }
            div id=(help) class="form-text" { "Markdown supported: **bold**, *italic*, # heading, - list, and [link](https://example.com)." }
            div id=(preview) class="markdown-preview notes-content border rounded p-3 mt-2" role="region" aria-label="Notes preview" aria-live="polite" hidden {}
        }
    }
}
