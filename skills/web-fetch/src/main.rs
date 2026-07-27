use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use scraper::{Html, Selector, ElementRef};
use serde::Deserialize;
use serde_json::{json, Value};
use skillforge_runtime::{dispatch, Embedded, SkillHandler};
use std::collections::HashMap;
use std::time::Duration;

// ─── Input types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Input {
    url: String,
    #[serde(default)]
    options: Options,
}

#[derive(Debug, Deserialize)]
struct Options {
    #[serde(default = "default_format")]
    format: Format,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
    #[serde(default = "default_max_length")]
    max_length: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: default_format(),
            headers: HashMap::new(),
            timeout_ms: default_timeout(),
            max_length: default_max_length(),
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Format {
    Raw,
    Text,
    #[default]
    Markdown,
}

fn default_format() -> Format {
    Format::Markdown
}
fn default_timeout() -> u64 {
    30000
}
fn default_max_length() -> usize {
    100000
}

// ─── Handler ────────────────────────────────────────────────────────────────

struct Handler;

impl SkillHandler for Handler {
    fn call(&self, input: Value) -> Result<Value> {
        let parsed: Input =
            serde_json::from_value(input).context("invalid input: expected {url, options?}")?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime")?;

        rt.block_on(fetch_url(parsed))
    }
}

async fn fetch_url(input: Input) -> Result<Value> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(input.options.timeout_ms))
        .user_agent("skillforge-web-fetch/0.1")
        .build()
        .context("failed to build HTTP client")?;

    let mut headers = HeaderMap::new();
    for (k, v) in &input.options.headers {
        let name: HeaderName = k
            .parse()
            .with_context(|| format!("invalid header name: {k}"))?;
        let value: HeaderValue = v
            .parse()
            .with_context(|| format!("invalid header value for {k}"))?;
        headers.insert(name, value);
    }

    let response = match client.get(&input.url).headers(headers).send().await {
        Ok(r) => r,
        Err(e) => {
            let msg = if e.is_timeout() {
                format!("request timed out after {}ms", input.options.timeout_ms)
            } else if e.is_connect() {
                format!("connection failed: {e}")
            } else {
                format!("request failed: {e}")
            };
            return Ok(json!({ "ok": false, "error": msg }));
        }
    };

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let body = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            return Ok(json!({ "ok": false, "error": format!("failed to read body: {e}") }));
        }
    };

    let is_html = content_type.contains("text/html") || content_type.contains("application/xhtml");

    let content = match input.options.format {
        Format::Raw => body,
        Format::Text => {
            if is_html {
                html_to_text(&body)
            } else {
                body
            }
        }
        Format::Markdown => {
            if is_html {
                html_to_markdown(&body)
            } else {
                body
            }
        }
    };

    let max_len = input.options.max_length;
    let truncated = content.len() > max_len;
    let content = if truncated {
        truncate_str(&content, max_len)
    } else {
        content
    };
    let content_length = content.len();

    Ok(json!({
        "ok": true,
        "data": {
            "url": input.url,
            "status": status,
            "content_type": content_type,
            "content": content,
            "content_length": content_length,
            "truncated": truncated,
        }
    }))
}

/// Truncate a string at a char boundary, keeping at most `max` bytes.
fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

// ─── HTML to plain text ─────────────────────────────────────────────────────

fn html_to_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    let mut text = String::new();
    let root = doc.root_element();
    extract_text_recursive(&root, &mut text, true);
    collapse_whitespace(&text)
}

fn extract_text_recursive(el: &ElementRef, buf: &mut String, skip_boilerplate: bool) {
    for child in el.children() {
        if let Some(element) = child.value().as_element() {
            let tag = element.name();
            if skip_boilerplate && is_boilerplate_tag(tag) {
                continue;
            }
            if tag == "script" || tag == "style" {
                continue;
            }
            if let Some(child_ref) = ElementRef::wrap(child) {
                if is_block_tag(tag) {
                    buf.push('\n');
                }
                extract_text_recursive(&child_ref, buf, skip_boilerplate);
                if is_block_tag(tag) {
                    buf.push('\n');
                }
            }
        } else if let Some(text_node) = child.value().as_text() {
            buf.push_str(text_node);
        }
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_blank = false;
    for line in s.lines() {
        let trimmed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if trimmed.is_empty() {
            if !prev_blank {
                result.push('\n');
                prev_blank = true;
            }
        } else {
            result.push_str(&trimmed);
            result.push('\n');
            prev_blank = false;
        }
    }
    result.trim().to_string()
}

// ─── HTML to markdown ───────────────────────────────────────────────────────

fn html_to_markdown(html: &str) -> String {
    let doc = Html::parse_document(html);
    let root = doc.root_element();
    let mut ctx = MdContext::default();
    convert_element(&root, &mut ctx);
    normalize_markdown(&ctx.output)
}

#[derive(Default)]
struct MdContext {
    output: String,
    list_stack: Vec<ListType>,
    list_index: Vec<usize>,
    in_pre: bool,
}

#[derive(Clone, Copy)]
enum ListType {
    Unordered,
    Ordered,
}

fn convert_element(el: &ElementRef, ctx: &mut MdContext) {
    for child in el.children() {
        if let Some(element) = child.value().as_element() {
            let tag = element.name();

            // Skip boilerplate and invisible elements
            if is_boilerplate_tag(tag) || tag == "script" || tag == "style" {
                continue;
            }

            if let Some(child_ref) = ElementRef::wrap(child) {
                match tag {
                    "h1" => write_heading(&child_ref, ctx, 1),
                    "h2" => write_heading(&child_ref, ctx, 2),
                    "h3" => write_heading(&child_ref, ctx, 3),
                    "h4" => write_heading(&child_ref, ctx, 4),
                    "h5" => write_heading(&child_ref, ctx, 5),
                    "h6" => write_heading(&child_ref, ctx, 6),
                    "p" => write_paragraph(&child_ref, ctx),
                    "br" => ctx.output.push('\n'),
                    "hr" => {
                        ensure_blank_line(&mut ctx.output);
                        ctx.output.push_str("---\n\n");
                    }
                    "a" => write_link(&child_ref, ctx),
                    "strong" | "b" => write_inline_wrap(&child_ref, ctx, "**"),
                    "em" | "i" => write_inline_wrap(&child_ref, ctx, "*"),
                    "code" => {
                        if !ctx.in_pre {
                            ctx.output.push('`');
                            let text = child_ref.text().collect::<String>();
                            ctx.output.push_str(&text);
                            ctx.output.push('`');
                        } else {
                            convert_element(&child_ref, ctx);
                        }
                    }
                    "pre" => write_code_block(&child_ref, ctx),
                    "blockquote" => write_blockquote(&child_ref, ctx),
                    "ul" => write_list(&child_ref, ctx, ListType::Unordered),
                    "ol" => write_list(&child_ref, ctx, ListType::Ordered),
                    "li" => write_list_item(&child_ref, ctx),
                    "table" => write_table(&child_ref, ctx),
                    "img" => write_image(&child_ref, ctx),
                    "div" | "section" | "article" | "main" | "aside" | "figure"
                    | "figcaption" | "details" | "summary" => {
                        convert_element(&child_ref, ctx);
                    }
                    "span" | "abbr" | "mark" | "small" | "sub" | "sup" | "time" => {
                        convert_element(&child_ref, ctx);
                    }
                    _ => {
                        convert_element(&child_ref, ctx);
                    }
                }
            }
        } else if let Some(text_node) = child.value().as_text() {
            if ctx.in_pre {
                ctx.output.push_str(text_node);
            } else {
                let text: &str = text_node;
                let collapsed = collapse_inline_whitespace(text);
                if !collapsed.is_empty() {
                    ctx.output.push_str(&collapsed);
                }
            }
        }
    }
}

fn write_heading(el: &ElementRef, ctx: &mut MdContext, level: u8) {
    ensure_blank_line(&mut ctx.output);
    for _ in 0..level {
        ctx.output.push('#');
    }
    ctx.output.push(' ');
    convert_element(el, ctx);
    ctx.output.push_str("\n\n");
}

fn write_paragraph(el: &ElementRef, ctx: &mut MdContext) {
    ensure_blank_line(&mut ctx.output);
    convert_element(el, ctx);
    ctx.output.push_str("\n\n");
}

fn write_link(el: &ElementRef, ctx: &mut MdContext) {
    let href = el.value().attr("href").unwrap_or("");
    ctx.output.push('[');
    convert_element(el, ctx);
    ctx.output.push_str("](");
    ctx.output.push_str(href);
    ctx.output.push(')');
}

fn write_inline_wrap(el: &ElementRef, ctx: &mut MdContext, marker: &str) {
    ctx.output.push_str(marker);
    convert_element(el, ctx);
    ctx.output.push_str(marker);
}

fn write_code_block(el: &ElementRef, ctx: &mut MdContext) {
    ensure_blank_line(&mut ctx.output);
    // Try to detect language from class
    let lang = el
        .select(&Selector::parse("code").unwrap())
        .next()
        .and_then(|code| {
            code.value().attr("class").and_then(|c| {
                c.split_whitespace()
                    .find(|cls| cls.starts_with("language-") || cls.starts_with("lang-"))
                    .map(|cls| {
                        cls.strip_prefix("language-")
                            .or_else(|| cls.strip_prefix("lang-"))
                            .unwrap_or("")
                    })
            })
        })
        .unwrap_or("");

    ctx.output.push_str("```");
    ctx.output.push_str(lang);
    ctx.output.push('\n');
    ctx.in_pre = true;
    convert_element(el, ctx);
    ctx.in_pre = false;
    if !ctx.output.ends_with('\n') {
        ctx.output.push('\n');
    }
    ctx.output.push_str("```\n\n");
}

fn write_blockquote(el: &ElementRef, ctx: &mut MdContext) {
    ensure_blank_line(&mut ctx.output);
    let mut inner_ctx = MdContext::default();
    convert_element(el, &mut inner_ctx);
    for line in inner_ctx.output.trim().lines() {
        ctx.output.push_str("> ");
        ctx.output.push_str(line);
        ctx.output.push('\n');
    }
    ctx.output.push('\n');
}

fn write_list(el: &ElementRef, ctx: &mut MdContext, list_type: ListType) {
    ensure_blank_line(&mut ctx.output);
    ctx.list_stack.push(list_type);
    ctx.list_index.push(0);
    convert_element(el, ctx);
    ctx.list_stack.pop();
    ctx.list_index.pop();
    if ctx.list_stack.is_empty() {
        ctx.output.push('\n');
    }
}

fn write_list_item(el: &ElementRef, ctx: &mut MdContext) {
    let depth = ctx.list_stack.len().saturating_sub(1);
    let indent = "  ".repeat(depth);
    let list_type = ctx.list_stack.last().copied().unwrap_or(ListType::Unordered);

    let marker = match list_type {
        ListType::Unordered => "- ".to_string(),
        ListType::Ordered => {
            if let Some(idx) = ctx.list_index.last_mut() {
                *idx += 1;
                format!("{}. ", *idx)
            } else {
                "1. ".to_string()
            }
        }
    };

    ctx.output.push_str(&indent);
    ctx.output.push_str(&marker);

    let mut item_ctx = MdContext {
        list_stack: ctx.list_stack.clone(),
        list_index: ctx.list_index.clone(),
        ..Default::default()
    };
    convert_element(el, &mut item_ctx);

    let text = item_ctx.output.trim().replace('\n', &format!("\n{}{}", indent, " ".repeat(marker.len())));
    ctx.output.push_str(&text);
    ctx.output.push('\n');
}

fn write_table(el: &ElementRef, ctx: &mut MdContext) {
    ensure_blank_line(&mut ctx.output);

    let tr_sel = Selector::parse("tr").unwrap();
    let th_sel = Selector::parse("th").unwrap();
    let td_sel = Selector::parse("td").unwrap();

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut has_header = false;

    for tr in el.select(&tr_sel) {
        let ths: Vec<String> = tr.select(&th_sel).map(|c| cell_text(&c)).collect();
        if !ths.is_empty() {
            has_header = true;
            rows.push(ths);
        } else {
            let tds: Vec<String> = tr.select(&td_sel).map(|c| cell_text(&c)).collect();
            if !tds.is_empty() {
                rows.push(tds);
            }
        }
    }

    if rows.is_empty() {
        return;
    }

    // Determine column widths
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return;
    }

    // Normalize row lengths
    for row in &mut rows {
        row.resize(cols, String::new());
    }

    let widths: Vec<usize> = (0..cols)
        .map(|i| rows.iter().map(|r| r[i].len()).max().unwrap_or(3).max(3))
        .collect();

    // Write header row
    let header_idx = 0;
    write_table_row(&rows[header_idx], &widths, &mut ctx.output);

    // Separator
    ctx.output.push('|');
    for w in &widths {
        ctx.output.push(' ');
        for _ in 0..*w {
            ctx.output.push('-');
        }
        ctx.output.push_str(" |");
    }
    ctx.output.push('\n');

    // Data rows
    let start = if has_header { 1 } else { 0 };
    for row in &rows[start..] {
        write_table_row(row, &widths, &mut ctx.output);
    }
    ctx.output.push('\n');
}

fn write_table_row(row: &[String], widths: &[usize], output: &mut String) {
    output.push('|');
    for (i, cell) in row.iter().enumerate() {
        output.push(' ');
        output.push_str(cell);
        let padding = widths[i].saturating_sub(cell.len());
        for _ in 0..padding {
            output.push(' ');
        }
        output.push_str(" |");
    }
    output.push('\n');
}

fn cell_text(el: &ElementRef) -> String {
    el.text().collect::<String>().trim().replace('\n', " ")
}

fn write_image(_el: &ElementRef, ctx: &mut MdContext) {
    let alt = _el.value().attr("alt").unwrap_or("");
    let src = _el.value().attr("src").unwrap_or("");
    if !src.is_empty() {
        ctx.output.push_str("![");
        ctx.output.push_str(alt);
        ctx.output.push_str("](");
        ctx.output.push_str(src);
        ctx.output.push(')');
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn is_boilerplate_tag(tag: &str) -> bool {
    matches!(tag, "nav" | "header" | "footer" | "aside")
}

fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "li"
            | "table" | "tr" | "blockquote" | "pre" | "section" | "article" | "main"
            | "figure" | "figcaption" | "hr" | "br"
    )
}

fn collapse_inline_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut last_was_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !last_was_ws {
                result.push(' ');
                last_was_ws = true;
            }
        } else {
            result.push(ch);
            last_was_ws = false;
        }
    }
    result
}

fn ensure_blank_line(s: &mut String) {
    if s.is_empty() {
        return;
    }
    if !s.ends_with('\n') {
        s.push_str("\n\n");
    } else if !s.ends_with("\n\n") {
        s.push('\n');
    }
}

fn normalize_markdown(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut blank_count = 0;
    for line in s.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
        }
    }
    result.trim().to_string()
}

// ─── Entrypoint ─────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let embedded = Embedded {
        manifest_toml: include_str!(concat!(env!("OUT_DIR"), "/skill.toml")),
        prompt_md: include_str!(concat!(env!("OUT_DIR"), "/prompt.md")),
        schema_json: include_str!(concat!(env!("OUT_DIR"), "/schema.json")),
    };
    dispatch(embedded, Handler)
}
