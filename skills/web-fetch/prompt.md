# web-fetch

Fetch a URL and return its content as text or simplified markdown, optimized for LLM consumption.

## Usage

```json
{
  "url": "https://example.com/article",
  "options": {
    "format": "markdown",
    "timeout_ms": 30000,
    "max_length": 100000
  }
}
```

## Parameters

- **url** (required, string): The URL to fetch. Must be a valid HTTP or HTTPS URL.
- **options** (optional, object): Configuration for the fetch operation.

### Options

| Field        | Type    | Default    | Description                                                   |
|--------------|---------|------------|---------------------------------------------------------------|
| format       | string  | "markdown" | Output format: `raw`, `text`, or `markdown`                   |
| headers      | object  | {}         | Additional HTTP headers (key-value string pairs)              |
| timeout_ms   | integer | 30000      | Request timeout in milliseconds                               |
| max_length   | integer | 100000     | Max content length in characters; excess content is truncated |

## Format Modes

### `raw`
Returns the response body exactly as received, truncated to `max_length` characters. Useful for APIs returning JSON, plain text, or when you need the unmodified response.

### `text`
Strips all HTML tags and collapses whitespace into clean plain text. Removes script, style, nav, header, and footer elements before extraction. Good for simple content extraction without formatting.

### `markdown`
Converts HTML into simplified markdown optimized for LLM reading:
- Headings (`<h1>`-`<h6>`) become `#` markdown headings
- Paragraphs get blank line separation
- Links become `[text](url)`
- Lists (`<ul>`, `<ol>`, `<li>`) become bulleted/numbered lists
- Code blocks (`<pre>`, `<code>`) become fenced code blocks
- Bold/italic are preserved with `**` and `*`
- Tables become markdown tables
- Boilerplate elements (nav, script, style, header, footer) are stripped

For non-HTML content types (JSON, plain text, etc.), the raw content is returned regardless of format setting.

## Truncation

When content exceeds `max_length` characters, it is truncated and `"truncated": true` is set in the response. The `content_length` field always reflects the length of the returned (possibly truncated) content.

## Response Format

### Success
```json
{
  "ok": true,
  "data": {
    "url": "https://example.com/article",
    "status": 200,
    "content_type": "text/html; charset=utf-8",
    "content": "# Article Title\n\nArticle content...",
    "content_length": 1234,
    "truncated": false
  }
}
```

### Error
```json
{
  "ok": false,
  "error": "request timed out after 30000ms"
}
```

## Examples

### Fetch a webpage as markdown
```json
{"url": "https://example.com"}
```

### Fetch raw JSON from an API
```json
{"url": "https://api.example.com/data", "options": {"format": "raw", "headers": {"Authorization": "Bearer token"}}}
```

### Fetch with short timeout and limited length
```json
{"url": "https://example.com/long-page", "options": {"timeout_ms": 5000, "max_length": 10000}}
```
