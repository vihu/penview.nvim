//! Shared preview page template.

use askama::Template;

#[derive(Debug, Template)]
#[template(path = "page.html")]
pub struct PageTemplate {
    pub title: String,
    pub body: String,
    pub use_websocket: bool,
    pub theme: String,
}

#[cfg(test)]
mod tests {
    use super::PageTemplate;
    use askama::Template;

    #[test]
    fn syntax_stylesheets_match_initial_page_theme() {
        for theme in ["light", "dark"] {
            for use_websocket in [false, true] {
                let html = PageTemplate {
                    title: "Code preview".to_string(),
                    body: "<pre><code>plain text</code></pre>".to_string(),
                    use_websocket,
                    theme: theme.to_string(),
                }
                .render()
                .unwrap();

                for (id, stylesheet, disabled) in [
                    ("highlight-light", "default.min.css", theme == "dark"),
                    ("highlight-dark", "github-dark.min.css", theme != "dark"),
                ] {
                    let link = html
                        .lines()
                        .find(|line| line.contains(&format!("id=\"{id}\"")))
                        .unwrap();
                    assert!(link.contains(&format!("/11.9.0/styles/{stylesheet}")));
                    assert_eq!(link.contains("disabled"), disabled, "{id} in {theme}");
                }
            }
        }
    }
}
