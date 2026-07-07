# http-client

Generic HTTP/REST client for making API requests with full control over method, headers, authentication, and body.

## Parameters

| Parameter    | Type    | Required | Description                                                                 |
|--------------|---------|----------|-----------------------------------------------------------------------------|
| `method`     | string  | No       | HTTP method: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`. Default: `GET` |
| `url`        | string  | Yes      | The full URL to send the request to                                         |
| `headers`    | object  | No       | HTTP headers as key-value string pairs                                      |
| `query`      | object  | No       | Query parameters as key-value string pairs (appended to the URL)            |
| `body`       | any     | No       | Request body: a JSON object (sent as `application/json`) or a string        |
| `auth`       | object  | No       | Authentication config (see below)                                           |
| `timeout_ms` | integer | No       | Request timeout in milliseconds. Default: `30000`                           |

### Authentication (`auth`)

| Field      | Type   | Description                                |
|------------|--------|--------------------------------------------|
| `type`     | string | `"bearer"` or `"basic"` (required)         |
| `token`    | string | Bearer token (when `type` is `"bearer"`)   |
| `username` | string | Username (when `type` is `"basic"`)        |
| `password` | string | Password (when `type` is `"basic"`)        |

## Examples

### GET with query parameters

```json
{
  "method": "GET",
  "url": "https://api.example.com/users",
  "query": {
    "page": "1",
    "limit": "20"
  },
  "headers": {
    "Accept": "application/json"
  }
}
```

### POST with JSON body and Bearer auth

```json
{
  "method": "POST",
  "url": "https://api.example.com/items",
  "headers": {
    "Content-Type": "application/json"
  },
  "body": {
    "name": "New Item",
    "description": "Created via http-client skill"
  },
  "auth": {
    "type": "bearer",
    "token": "eyJhbGciOiJIUzI1NiIs..."
  }
}
```

### PUT with Basic auth

```json
{
  "method": "PUT",
  "url": "https://api.example.com/items/42",
  "body": {
    "name": "Updated Item"
  },
  "auth": {
    "type": "basic",
    "username": "admin",
    "password": "secret"
  },
  "timeout_ms": 10000
}
```

### DELETE request

```json
{
  "method": "DELETE",
  "url": "https://api.example.com/items/42",
  "auth": {
    "type": "bearer",
    "token": "eyJhbGciOiJIUzI1NiIs..."
  }
}
```

## Response Format

### Success

```json
{
  "ok": true,
  "data": {
    "status": 200,
    "headers": {
      "content-type": "application/json",
      "x-request-id": "abc123"
    },
    "body": "{\"id\":1,\"name\":\"Example\"}"
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

## Notes

- Response `body` is always returned as a string. Parse JSON responses on the caller side.
- Response `headers` are returned with lowercase keys (HTTP/2 convention).
- If `body` is a JSON object and no `Content-Type` header is set, `application/json` is used automatically.
- The skill does not follow redirects beyond the default limit (10 hops).
- TLS is handled via rustls (no OpenSSL dependency).
