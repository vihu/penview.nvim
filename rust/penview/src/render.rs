use crate::{page_template::PageTemplate, svg_template::SvgTemplate};
use askama::Template;
use base64::{Engine, engine::general_purpose};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Tag, TagEnd};
use resolve_path::PathResolveExt;
use std::path::{Path, PathBuf};
use url::Url;

use tokio::fs::{read, read_to_string};

fn data_url(data: &[u8], mime_type: &str) -> String {
    let encoded = general_purpose::STANDARD.encode(data);

    format!("data:{};base64,{encoded}", mime_type)
}

/// Converts a byte offset to a 1-based line number
fn byte_offset_to_line(content: &str, offset: usize) -> usize {
    content[..offset.min(content.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// Gets the file at a specified path, loads it, and converts it to a base64-encoded data URL
async fn path_to_data_url(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let file = read(&path).await?;

    Ok(data_url(
        &file,
        mime_guess::from_path(&path)
            .first_raw()
            .unwrap_or("text/plain"),
    ))
}

/// Generates an SVG image containing a message and serializes it to a data URL.
fn generate_message_data_url(message: impl AsRef<str>, color: impl AsRef<str>) -> String {
    data_url(
        SvgTemplate {
            fill: color.as_ref().to_string(),
            text: message.as_ref().to_string(),
        }
        .to_string()
        .as_bytes(),
        "image/svg+xml",
    )
}

/// Renders a file to an HTML string.
///
/// `use_websocket` determines whether to include code for automatically updating the document with a
/// WebSocket connection.
pub async fn render_doc(path: impl AsRef<Path>, use_websocket: bool) -> anyhow::Result<String> {
    let path = path.as_ref().canonicalize()?;
    let file = read_to_string(&path).await?;
    let title = path.as_os_str().to_string_lossy().to_string();

    let body = render_markdown_to_html(&file, &path).await;

    let template = PageTemplate {
        body,
        title,
        use_websocket,
    };

    Ok(template.render().unwrap())
}

/// Renders markdown content from a string to HTML body.
/// Used for live preview where content comes from Neovim buffer.
///
/// `base_path` is used to resolve relative image paths.
pub async fn render_content(content: &str, base_path: &Path) -> anyhow::Result<String> {
    Ok(render_markdown_to_html(content, base_path).await)
}

/// Core markdown rendering logic shared by render_doc and render_content.
/// Generates HTML with data-source-line attributes for scroll synchronization.
async fn render_markdown_to_html(content: &str, base_path: &Path) -> String {
    let options = pulldown_cmark::Options::all();
    let parser = pulldown_cmark::Parser::new_ext(content, options);
    let mut events: Vec<_> = parser.into_offset_iter().collect();

    // Handle URLs - rewrite image and link URLs
    for (event, _range) in events.iter_mut() {
        // Resolve image links asynchronously
        if let Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url,
            ..
        }) = event
        {
            if dest_url.parse::<Url>().is_ok() {
                continue;
            } else {
                let image_path: PathBuf = dest_url.parse().unwrap();
                *dest_url = path_to_data_url(image_path.resolve_in(base_path))
                    .await
                    .unwrap_or(generate_message_data_url("Disk error.", "red"))
                    .into()
            }
        }

        // Rewrite URLs to open links
        if let Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url,
            ..
        }) = event
        {
            // If the link is a valid URL, leave it
            if dest_url.parse::<Url>().is_err() {
                // Otherwise, try to parse it as a file path
                let file_path: PathBuf = dest_url.parse().unwrap();
                // If it's a filepath check if it's relative
                if let Some(file_path) = file_path.to_str() {
                    let file_path = if Path::new(file_path).is_relative() {
                        // If it's relative, join it to the current file
                        join_and_canonicalize(file_path, base_path.to_path_buf())
                            .unwrap_or(file_path.into())
                    } else {
                        // Otherwise, use the file path as is
                        file_path.into()
                    };

                    // If possible, return a relative path from the cwd
                    let file_path = match get_relative_path_under_cwd(file_path.clone()) {
                        Some(path) => path,
                        None => file_path,
                    };
                    *dest_url = format!("/?path={}", file_path.to_str().unwrap()).into()
                }
            }
        }
    }

    // Custom HTML generation with data-source-line attributes
    let mut body = String::new();
    let mut in_code_block = false;

    for (event, range) in events {
        let line = byte_offset_to_line(content, range.start);

        match event {
            Event::Start(tag) => {
                write_start_tag(&mut body, &tag, line, &mut in_code_block);
            }
            Event::End(tag_end) => {
                write_end_tag(&mut body, &tag_end, &mut in_code_block);
            }
            Event::Text(text) => {
                body.push_str(&escape_html(&text));
            }
            Event::Code(code) => {
                body.push_str("<code>");
                body.push_str(&escape_html(&code));
                body.push_str("</code>");
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                body.push_str(&html);
            }
            Event::SoftBreak => {
                body.push('\n');
            }
            Event::HardBreak => {
                body.push_str("<br />\n");
            }
            Event::Rule => {
                body.push_str(&format!("<hr data-source-line=\"{}\" />\n", line));
            }
            Event::FootnoteReference(name) => {
                body.push_str(&format!(
                    "<sup class=\"footnote-reference\"><a href=\"#{}\">{}</a></sup>",
                    escape_html(&name),
                    escape_html(&name)
                ));
            }
            Event::TaskListMarker(checked) => {
                if checked {
                    body.push_str("<input type=\"checkbox\" disabled checked />");
                } else {
                    body.push_str("<input type=\"checkbox\" disabled />");
                }
            }
            Event::InlineMath(math) => {
                body.push_str("<span class=\"math\">");
                body.push_str(&escape_html(&math));
                body.push_str("</span>");
            }
            Event::DisplayMath(math) => {
                body.push_str(&format!(
                    "<div class=\"math\" data-source-line=\"{}\">",
                    line
                ));
                body.push_str(&escape_html(&math));
                body.push_str("</div>\n");
            }
        }
    }

    body
}

/// Escape HTML special characters
fn escape_html(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#x27;"),
            _ => result.push(c),
        }
    }
    result
}

/// Convert HeadingLevel to u8
fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Write opening tag with data-source-line attribute for block elements
fn write_start_tag(output: &mut String, tag: &Tag, line: usize, in_code_block: &mut bool) {
    match tag {
        Tag::Paragraph => {
            output.push_str(&format!("<p data-source-line=\"{}\">", line));
        }
        Tag::Heading { level, id, .. } => {
            let level_num = heading_level_to_u8(*level);
            if let Some(id) = id {
                output.push_str(&format!(
                    "<h{} id=\"{}\" data-source-line=\"{}\">",
                    level_num,
                    escape_html(id),
                    line
                ));
            } else {
                output.push_str(&format!("<h{} data-source-line=\"{}\">", level_num, line));
            }
        }
        Tag::BlockQuote(_) => {
            output.push_str(&format!("<blockquote data-source-line=\"{}\">\n", line));
        }
        Tag::CodeBlock(kind) => {
            *in_code_block = true;
            match kind {
                CodeBlockKind::Fenced(info) => {
                    let lang = info.split_whitespace().next().unwrap_or("");
                    if lang.is_empty() {
                        output.push_str(&format!("<pre data-source-line=\"{}\"><code>", line));
                    } else {
                        output.push_str(&format!(
                            "<pre data-source-line=\"{}\"><code class=\"language-{}\">",
                            line,
                            escape_html(lang)
                        ));
                    }
                }
                CodeBlockKind::Indented => {
                    output.push_str(&format!("<pre data-source-line=\"{}\"><code>", line));
                }
            }
        }
        Tag::List(start) => {
            if let Some(start_num) = start {
                if *start_num == 1 {
                    output.push_str(&format!("<ol data-source-line=\"{}\">\n", line));
                } else {
                    output.push_str(&format!(
                        "<ol start=\"{}\" data-source-line=\"{}\">\n",
                        start_num, line
                    ));
                }
            } else {
                output.push_str(&format!("<ul data-source-line=\"{}\">\n", line));
            }
        }
        Tag::Item => {
            output.push_str(&format!("<li data-source-line=\"{}\">", line));
        }
        Tag::FootnoteDefinition(name) => {
            output.push_str(&format!(
                "<div class=\"footnote-definition\" id=\"{}\" data-source-line=\"{}\">\n<span class=\"footnote-definition-label\">{}</span>\n",
                escape_html(name),
                line,
                escape_html(name)
            ));
        }
        Tag::Table(alignments) => {
            output.push_str(&format!("<table data-source-line=\"{}\">\n", line));
            // Store alignments for later use (we'd need state, but for now just ignore)
            let _ = alignments;
        }
        Tag::TableHead => {
            output.push_str("<thead>\n<tr>\n");
        }
        Tag::TableRow => {
            output.push_str("<tr>\n");
        }
        Tag::TableCell => {
            output.push_str("<td>");
        }
        Tag::Emphasis => {
            output.push_str("<em>");
        }
        Tag::Strong => {
            output.push_str("<strong>");
        }
        Tag::Strikethrough => {
            output.push_str("<del>");
        }
        Tag::Superscript => {
            output.push_str("<sup>");
        }
        Tag::Subscript => {
            output.push_str("<sub>");
        }
        Tag::Link {
            dest_url, title, ..
        } => {
            output.push_str("<a href=\"");
            output.push_str(&escape_html(dest_url));
            output.push('"');
            if !title.is_empty() {
                output.push_str(" title=\"");
                output.push_str(&escape_html(title));
                output.push('"');
            }
            output.push('>');
        }
        Tag::Image { dest_url, .. } => {
            output.push_str("<img src=\"");
            output.push_str(&escape_html(dest_url));
            output.push_str("\" alt=\"");
            // Alt text will be filled by Text event, closing tag adds title and />
        }
        Tag::MetadataBlock(_) => {
            // Skip metadata blocks
        }
        Tag::DefinitionList => {
            output.push_str(&format!("<dl data-source-line=\"{}\">\n", line));
        }
        Tag::DefinitionListTitle => {
            output.push_str(&format!("<dt data-source-line=\"{}\">", line));
        }
        Tag::DefinitionListDefinition => {
            output.push_str(&format!("<dd data-source-line=\"{}\">", line));
        }
        Tag::HtmlBlock => {
            // HTML blocks are passed through as-is
        }
    }
}

/// Write closing tag
fn write_end_tag(output: &mut String, tag_end: &TagEnd, in_code_block: &mut bool) {
    match tag_end {
        TagEnd::Paragraph => {
            output.push_str("</p>\n");
        }
        TagEnd::Heading(level) => {
            let level_num = heading_level_to_u8(*level);
            output.push_str(&format!("</h{}>\n", level_num));
        }
        TagEnd::BlockQuote(_) => {
            output.push_str("</blockquote>\n");
        }
        TagEnd::CodeBlock => {
            *in_code_block = false;
            output.push_str("</code></pre>\n");
        }
        TagEnd::List(ordered) => {
            if *ordered {
                output.push_str("</ol>\n");
            } else {
                output.push_str("</ul>\n");
            }
        }
        TagEnd::Item => {
            output.push_str("</li>\n");
        }
        TagEnd::FootnoteDefinition => {
            output.push_str("</div>\n");
        }
        TagEnd::Table => {
            output.push_str("</tbody>\n</table>\n");
        }
        TagEnd::TableHead => {
            output.push_str("</tr>\n</thead>\n<tbody>\n");
        }
        TagEnd::TableRow => {
            output.push_str("</tr>\n");
        }
        TagEnd::TableCell => {
            output.push_str("</td>\n");
        }
        TagEnd::Emphasis => {
            output.push_str("</em>");
        }
        TagEnd::Strong => {
            output.push_str("</strong>");
        }
        TagEnd::Strikethrough => {
            output.push_str("</del>");
        }
        TagEnd::Superscript => {
            output.push_str("</sup>");
        }
        TagEnd::Subscript => {
            output.push_str("</sub>");
        }
        TagEnd::Link => {
            output.push_str("</a>");
        }
        TagEnd::Image => {
            output.push_str("\" />");
        }
        TagEnd::MetadataBlock(_) => {
            // Skip metadata blocks
        }
        TagEnd::DefinitionList => {
            output.push_str("</dl>\n");
        }
        TagEnd::DefinitionListTitle => {
            output.push_str("</dt>\n");
        }
        TagEnd::DefinitionListDefinition => {
            output.push_str("</dd>\n");
        }
        TagEnd::HtmlBlock => {
            // HTML blocks are passed through as-is
        }
    }
}

/// Returns a relative path to a file if it is under the working directory
///
/// # Arguments
/// * `file_path` - A `PathBuf` representing the file or directory path to check and convert if necessary.
///
/// # Returns
/// * A `PathBuf` object containing either a relative or absolute path to the `file_path`.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// let file_path = std::env::current_dir().unwrap().join("file.txt");
/// let path = get_relative_or_absolute_path(file_path).unwrap();
/// println!("{:?}", path); // Outputs the relative or absolute path to "file.txt"
/// ```
fn get_relative_path_under_cwd(file_path: PathBuf) -> Option<PathBuf> {
    if let Ok(current_dir) = std::env::current_dir() {
        if is_child_path(current_dir, file_path.clone()) {
            truncate_cwd(&file_path)
        } else {
            Some(file_path)
        }
    } else {
        Some(file_path)
    }
}

/// Joins a relative link to a the directory of the current file and returns the canonical path
/// Unlike the std::path::PathBuf::canonicalize method, this function does not panic if the file does not exist.
///
/// This function is used to resolve the link to a target from a markdown file
/// It does not panic.
/// TODO it should return an enum and be unwrapped above
///
/// Note: The `current_file` input must correspond to a file path, not a directory path.
///
/// # Arguments
///
/// * `path` - A str of the relative file path to be converted.
/// * `current_file` - A PathBuf which holds the path to the current file.
///
/// # Return
///
/// This function returns a String that represents the absolute path of the target file.
///
/// ```
/// let current_file = PathBuf::from("/home/user/Notes/slipbox/networking/dns.md");
/// assert_eq!(rel_to_abspath("../linux.md", current_file), String::from("/home/user/Notes/slipbox/linux.md"));
/// ```
///
fn join_and_canonicalize(path: &str, current_file: PathBuf) -> std::io::Result<PathBuf> {
    let current_dir = current_file
        .parent()
        .ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No parent directory",
        ))?
        .to_path_buf()
        .join(path);

    // Clean up the path
    let mut clean_path = PathBuf::new();
    for component in current_dir.components() {
        match component {
            std::path::Component::ParentDir => {
                clean_path.pop();
            }
            std::path::Component::CurDir => continue,
            _ => clean_path.push(component),
        }
    }

    Ok(clean_path)
}

/// Takes an absolute path of a file under the current working directory
/// and returns a relative path with the current working directory removed.
/// If the input path does not start with the current working directory, or if there's an error retrieving the current working directory,
/// this function None
///
/// # Arguments
/// * `file_path` - A reference to a PathBuf object representing the absolute path from which to remove the current working directory.
///
/// # Returns
/// * A PathBuf object representing the relative path with the current working directory removed, or a copy of the input path if this is not possible.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// let path = PathBuf::from("/home/user/documents/file.txt");
/// let truncated_path = try_truncate_cwd(&path);
/// println!("{}", truncated_path.display().unwrap());
/// // Outputs "documents/file.txt" if current working directory is "/home/user"
/// ```
fn truncate_cwd(file_path: &PathBuf) -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    let file_path = PathBuf::from(file_path);
    Some(
        file_path
            .strip_prefix(&current_dir)
            .ok()?
            .iter()
            .map(|p| p.to_owned())
            .collect(),
    )
}

/// Takes two absolute paths and returns true if the second is a child of the first.
///
/// # Arguments
/// * `parent_dir` - A PathBuf object representing the parent directory.
/// * `child` - A PathBuf object representing the candidate child
///
/// # Returns
/// * A boolean value indicating whether the second path is a child of the first.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// let parent_dir = PathBuf::from("/home/user/Notes/slipbox/");
/// let child = PathBuf::from("/home/user/Notes/slipbox/child.md");
/// assert_eq!(is_child_path(parent_dir, child), true);
/// ```
fn is_child_path(parent_dir: PathBuf, child: PathBuf) -> bool {
    if child.is_relative() {
        return false;
    }

    // Get the components of both
    let parent_components: Vec<_> = parent_dir.components().collect();
    let child_components: Vec<_> = child.components().collect();

    // If the length of child's components is less than or equal to parent's, they cannot be a child path
    if child_components.len() <= parent_components.len() {
        return false;
    }

    // Truncate the child_components
    let child_components: Vec<_> = child_components[0..parent_components.len()].to_vec();

    for (p, c) in parent_components.iter().zip(child_components.iter()) {
        if p != c {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rel_to_abspath() {
        let current_file = PathBuf::from("/home/user/Notes/slipbox/networking/dns.md");
        assert_eq!(
            join_and_canonicalize("../linux.md", current_file.clone()).unwrap(),
            PathBuf::from("/home/user/Notes/slipbox/linux.md")
        );

        let current_dir = current_file.parent().unwrap().to_path_buf();

        // If passed a directory, we expect a mistaken link as the function
        // gets the directory with pop and requires the directory
        // to exist in order to test
        assert_eq!(
            join_and_canonicalize("../linux.md", current_dir).unwrap(),
            PathBuf::from("/home/user/Notes/linux.md")
        );

        // Also preserve relative paths
        assert_eq!(
            join_and_canonicalize("../linux.md", PathBuf::from("./networking/dns.md")).unwrap(),
            PathBuf::from("linux.md")
        );
    }

    #[test]
    fn test_truncate_cwd() {
        let file_path = std::env::current_dir().unwrap().join("file.md");
        assert_eq!(truncate_cwd(&file_path), Some(PathBuf::from("file.md")));

        let file_path = std::env::current_dir().unwrap().join("foo/bar/baz/file.md");
        assert_eq!(
            truncate_cwd(&file_path),
            Some(PathBuf::from("foo/bar/baz/file.md"))
        );
    }

    #[test]
    fn test_is_child_path() {
        let current_file: PathBuf = PathBuf::from("/home/user/Notes/slipbox/");
        let file: PathBuf = PathBuf::from("/home/user/Notes/slipbox/child.md");

        // The directory of the current file is the parent directory of the file
        // So this should return true
        assert_eq!(is_child_path(current_file, file), true);

        let current_file: PathBuf = PathBuf::from("/home/user/Notes/slipbox/");
        let file: PathBuf = PathBuf::from("/home/user/Notes/not_child.md");

        // The directory of the current file is the parent directory of the file
        // So this should return true
        assert_ne!(is_child_path(current_file, file), true);
    }

    #[test]
    fn test_get_relative_path_under_cwd() {
        let current_dir = std::env::current_dir().unwrap();
        let child_file = current_dir.join("child_file");
        assert_eq!(
            get_relative_path_under_cwd(child_file).unwrap(),
            PathBuf::from("child_file")
        );
    }
}
