use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Method};
use serde::Deserialize;
use serde_json::{json, Value};
use skillforge_runtime::{dispatch, Embedded, SkillHandler};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct Request {
    #[serde(default = "default_method")]
    method: String,
    url: String,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    query: Option<HashMap<String, String>>,
    #[serde(default)]
    body: Option<Value>,
    #[serde(default)]
    auth: Option<Auth>,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
struct Auth {
    #[serde(rename = "type")]
    auth_type: String,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

fn default_method() -> String {
    "GET".to_string()
}

fn default_timeout() -> u64 {
    30000
}

struct Handler;

impl SkillHandler for Handler {
    fn call(&self, input: Value) -> Result<Value> {
        let req: Request = serde_json::from_value(input).context("invalid input")?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("building tokio runtime")?;

        let result = rt.block_on(async move { execute_request(req).await });

        Ok(match result {
            Ok(data) => json!({ "ok": true, "data": data }),
            Err(e) => json!({ "ok": false, "error": format!("{e:#}") }),
        })
    }
}

fn main() -> Result<()> {
    let embedded = Embedded {
        manifest_toml: include_str!(concat!(env!("OUT_DIR"), "/skill.toml")),
        prompt_md: include_str!(concat!(env!("OUT_DIR"), "/prompt.md")),
        schema_json: include_str!(concat!(env!("OUT_DIR"), "/schema.json")),
    };
    dispatch(embedded, Handler)
}

async fn execute_request(req: Request) -> Result<Value> {
    let method = match req.method.to_uppercase().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "PATCH" => Method::PATCH,
        "DELETE" => Method::DELETE,
        "HEAD" => Method::HEAD,
        other => return Err(anyhow!("unsupported HTTP method: {other}")),
    };

    let client = Client::builder()
        .timeout(Duration::from_millis(req.timeout_ms))
        .build()
        .context("building HTTP client")?;

    let mut request = client.request(method, &req.url);

    // Apply query parameters
    if let Some(query) = req.query {
        request = request.query(&query.iter().collect::<Vec<_>>());
    }

    // Apply authentication
    if let Some(auth) = &req.auth {
        match auth.auth_type.as_str() {
            "bearer" => {
                let token = auth
                    .token
                    .as_deref()
                    .ok_or_else(|| anyhow!("auth type 'bearer' requires 'token' field"))?;
                request = request.bearer_auth(token);
            }
            "basic" => {
                let username = auth
                    .username
                    .as_deref()
                    .ok_or_else(|| anyhow!("auth type 'basic' requires 'username' field"))?;
                let password = auth.password.as_deref();
                request = request.basic_auth(username, password);
            }
            other => return Err(anyhow!("unsupported auth type: {other}")),
        }
    }

    // Apply headers
    let mut has_content_type = false;
    if let Some(headers) = &req.headers {
        for (key, value) in headers {
            if key.to_lowercase() == "content-type" {
                has_content_type = true;
            }
            request = request.header(key, value);
        }
    }

    // Apply body
    if let Some(body) = req.body {
        match body {
            Value::String(s) => {
                request = request.body(s);
            }
            Value::Object(_) | Value::Array(_) => {
                if !has_content_type {
                    request = request.header("content-type", "application/json");
                }
                request = request.body(serde_json::to_string(&body)?);
            }
            Value::Null => {}
            _ => {
                // Numbers, booleans - serialize as JSON
                if !has_content_type {
                    request = request.header("content-type", "application/json");
                }
                request = request.body(serde_json::to_string(&body)?);
            }
        }
    }

    let response = request.send().await.context("sending HTTP request")?;

    let status = response.status().as_u16();
    let response_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body_text = response.text().await.context("reading response body")?;

    Ok(json!({
        "status": status,
        "headers": response_headers,
        "body": body_text,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_invalid_input() {
        let handler = Handler;
        // Missing required "url" field - deserialization fails, call() returns Err
        let result = handler.call(json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_missing_url() {
        let handler = Handler;
        let result = handler.call(json!({"method": "GET"}));
        // Should error because url is required by serde
        assert!(result.is_err());
    }

    #[test]
    fn test_default_method() {
        assert_eq!(default_method(), "GET");
    }

    #[test]
    fn test_default_timeout() {
        assert_eq!(default_timeout(), 30000);
    }

    #[test]
    fn test_request_deserialization_minimal() {
        let input = json!({"url": "https://example.com"});
        let req: Request = serde_json::from_value(input).unwrap();
        assert_eq!(req.url, "https://example.com");
        assert_eq!(req.method, "GET");
        assert_eq!(req.timeout_ms, 30000);
        assert!(req.headers.is_none());
        assert!(req.query.is_none());
        assert!(req.body.is_none());
        assert!(req.auth.is_none());
    }

    #[test]
    fn test_request_deserialization_full() {
        let input = json!({
            "method": "POST",
            "url": "https://api.example.com/items",
            "headers": {"Content-Type": "application/json"},
            "query": {"page": "1"},
            "body": {"name": "test"},
            "auth": {"type": "bearer", "token": "abc123"},
            "timeout_ms": 5000
        });
        let req: Request = serde_json::from_value(input).unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.url, "https://api.example.com/items");
        assert_eq!(req.timeout_ms, 5000);
        let auth = req.auth.unwrap();
        assert_eq!(auth.auth_type, "bearer");
        assert_eq!(auth.token.unwrap(), "abc123");
    }

    #[test]
    fn test_auth_basic_deserialization() {
        let input = json!({
            "type": "basic",
            "username": "admin",
            "password": "secret"
        });
        let auth: Auth = serde_json::from_value(input).unwrap();
        assert_eq!(auth.auth_type, "basic");
        assert_eq!(auth.username.unwrap(), "admin");
        assert_eq!(auth.password.unwrap(), "secret");
    }
}
