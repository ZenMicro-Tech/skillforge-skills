use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use skillforge_runtime::{dispatch, Embedded, SkillHandler};
use std::env;

/// Input schema for the GitHub skill.
#[derive(Debug, Deserialize)]
struct Input {
    operation: String,
    #[serde(default)]
    args: Value,
}

struct Handler;

impl SkillHandler for Handler {
    fn call(&self, input: Value) -> Result<Value> {
        let req: Input = serde_json::from_value(input).context("invalid input")?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("building tokio runtime")?;

        let result = rt.block_on(async move { execute(req).await });

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

/// Resolve the GitHub personal access token from the environment.
fn get_token() -> Result<String> {
    env::var("GITHUB_TOKEN")
        .or_else(|_| env::var("GH_TOKEN"))
        .map_err(|_| anyhow!("neither GITHUB_TOKEN nor GH_TOKEN environment variable is set"))
}

/// Build a pre-configured reqwest client with auth and User-Agent.
fn build_client(token: &str) -> Result<Client> {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};

    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("skillforge-github/0.1.0"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static("2022-11-28"),
    );
    let auth_value = format!("Bearer {token}");
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_value).context("invalid token characters")?,
    );

    Client::builder()
        .default_headers(headers)
        .build()
        .context("building HTTP client")
}

/// Helper to extract a string arg from the args object.
fn arg_str(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| anyhow!("missing required argument: {key}"))
}

/// Helper to extract an optional string arg.
fn arg_str_opt(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(String::from)
}

/// Helper to extract an optional u64 arg.
fn arg_u64_opt(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(Value::as_u64)
}

/// Helper to extract a required integer arg.
fn arg_u64(args: &Value, key: &str) -> Result<u64> {
    args.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("missing required argument: {key}"))
}

const BASE_URL: &str = "https://api.github.com";

async fn execute(input: Input) -> Result<Value> {
    let token = get_token()?;
    let client = build_client(&token)?;
    let args = &input.args;

    match input.operation.as_str() {
        "get-repo" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}");
            api_get(&client, &url).await
        }

        "list-issues" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/issues");
            let mut query: Vec<(String, String)> = Vec::new();
            if let Some(state) = arg_str_opt(args, "state") {
                query.push(("state".into(), state));
            }
            if let Some(labels) = arg_str_opt(args, "labels") {
                query.push(("labels".into(), labels));
            }
            if let Some(per_page) = arg_u64_opt(args, "per_page") {
                query.push(("per_page".into(), per_page.to_string()));
            }
            api_get_query(&client, &url, &query).await
        }

        "get-issue" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let number = arg_u64(args, "number")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/issues/{number}");
            api_get(&client, &url).await
        }

        "create-issue" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let title = arg_str(args, "title")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/issues");
            let mut body_json = json!({ "title": title });
            if let Some(body) = arg_str_opt(args, "body") {
                body_json["body"] = json!(body);
            }
            if let Some(labels) = args.get("labels") {
                body_json["labels"] = labels.clone();
            }
            if let Some(assignees) = args.get("assignees") {
                body_json["assignees"] = assignees.clone();
            }
            api_post(&client, &url, body_json).await
        }

        "list-pulls" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/pulls");
            let mut query: Vec<(String, String)> = Vec::new();
            if let Some(state) = arg_str_opt(args, "state") {
                query.push(("state".into(), state));
            }
            if let Some(per_page) = arg_u64_opt(args, "per_page") {
                query.push(("per_page".into(), per_page.to_string()));
            }
            api_get_query(&client, &url, &query).await
        }

        "get-pull" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let number = arg_u64(args, "number")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/pulls/{number}");
            api_get(&client, &url).await
        }

        "create-pull" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let title = arg_str(args, "title")?;
            let head = arg_str(args, "head")?;
            let base = arg_str(args, "base")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/pulls");
            let mut body_json = json!({
                "title": title,
                "head": head,
                "base": base,
            });
            if let Some(body) = arg_str_opt(args, "body") {
                body_json["body"] = json!(body);
            }
            api_post(&client, &url, body_json).await
        }

        "list-pull-files" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let number = arg_u64(args, "number")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/pulls/{number}/files");
            api_get(&client, &url).await
        }

        "create-review-comment" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let number = arg_u64(args, "number")?;
            let body = arg_str(args, "body")?;
            let path = arg_str(args, "path")?;
            let line = arg_u64(args, "line")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/pulls/{number}/comments");
            let body_json = json!({
                "body": body,
                "path": path,
                "line": line,
            });
            api_post(&client, &url, body_json).await
        }

        "list-runs" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/actions/runs");
            let mut query: Vec<(String, String)> = Vec::new();
            if let Some(per_page) = arg_u64_opt(args, "per_page") {
                query.push(("per_page".into(), per_page.to_string()));
            }
            api_get_query(&client, &url, &query).await
        }

        "list-releases" => {
            let owner = arg_str(args, "owner")?;
            let repo = arg_str(args, "repo")?;
            let url = format!("{BASE_URL}/repos/{owner}/{repo}/releases");
            let mut query: Vec<(String, String)> = Vec::new();
            if let Some(per_page) = arg_u64_opt(args, "per_page") {
                query.push(("per_page".into(), per_page.to_string()));
            }
            api_get_query(&client, &url, &query).await
        }

        other => Err(anyhow!("unknown operation: {other}")),
    }
}

/// Perform a GET request and parse the JSON response.
async fn api_get(client: &Client, url: &str) -> Result<Value> {
    let resp = client.get(url).send().await.context("GET request failed")?;
    handle_response(resp).await
}

/// Perform a GET request with query parameters.
async fn api_get_query(client: &Client, url: &str, query: &[(String, String)]) -> Result<Value> {
    let resp = client
        .get(url)
        .query(query)
        .send()
        .await
        .context("GET request failed")?;
    handle_response(resp).await
}

/// Perform a POST request with a JSON body.
async fn api_post(client: &Client, url: &str, body: Value) -> Result<Value> {
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .context("POST request failed")?;
    handle_response(resp).await
}

/// Handle the HTTP response: check status, parse JSON body.
async fn handle_response(resp: reqwest::Response) -> Result<Value> {
    let status = resp.status();
    let body = resp.text().await.context("reading response body")?;

    if !status.is_success() {
        // Try to extract the GitHub error message
        if let Ok(err_json) = serde_json::from_str::<Value>(&body) {
            let message = err_json
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(anyhow!(
                "GitHub API returned {}: {}",
                status.as_u16(),
                message
            ));
        }
        return Err(anyhow!("GitHub API returned {}: {}", status.as_u16(), body));
    }

    serde_json::from_str(&body).context("parsing GitHub API response as JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_deserialization() {
        let input = json!({
            "operation": "get-repo",
            "args": { "owner": "octocat", "repo": "hello-world" }
        });
        let req: Input = serde_json::from_value(input).unwrap();
        assert_eq!(req.operation, "get-repo");
        assert_eq!(req.args["owner"], "octocat");
    }

    #[test]
    fn test_input_deserialization_no_args() {
        let input = json!({ "operation": "get-repo" });
        let req: Input = serde_json::from_value(input).unwrap();
        assert_eq!(req.operation, "get-repo");
        assert!(req.args.is_null());
    }

    #[test]
    fn test_arg_str_present() {
        let args = json!({ "owner": "octocat" });
        assert_eq!(arg_str(&args, "owner").unwrap(), "octocat");
    }

    #[test]
    fn test_arg_str_missing() {
        let args = json!({});
        assert!(arg_str(&args, "owner").is_err());
    }

    #[test]
    fn test_arg_str_opt_present() {
        let args = json!({ "state": "open" });
        assert_eq!(arg_str_opt(&args, "state"), Some("open".into()));
    }

    #[test]
    fn test_arg_str_opt_missing() {
        let args = json!({});
        assert_eq!(arg_str_opt(&args, "state"), None);
    }

    #[test]
    fn test_arg_u64_present() {
        let args = json!({ "number": 42 });
        assert_eq!(arg_u64(&args, "number").unwrap(), 42);
    }

    #[test]
    fn test_arg_u64_missing() {
        let args = json!({});
        assert!(arg_u64(&args, "number").is_err());
    }

    #[test]
    fn test_unknown_operation() {
        // We can't easily test the full async flow without a token, but we can
        // verify that the handler returns an error for unknown operations when
        // no token is set.
        let handler = Handler;
        let result = handler.call(json!({
            "operation": "unknown-op",
            "args": {}
        }));
        let val = result.unwrap();
        assert_eq!(val["ok"], false);
        // Error should mention either the token or the unknown operation
        let err = val["error"].as_str().unwrap();
        assert!(
            err.contains("GITHUB_TOKEN") || err.contains("unknown operation"),
            "unexpected error: {err}"
        );
    }
}
