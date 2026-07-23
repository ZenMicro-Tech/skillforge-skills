use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use skillforge_runtime::{dispatch, Embedded, SkillHandler};
use std::process::Command;

const MAX_LOG_LINES: u64 = 10_000;

struct Handler;

impl SkillHandler for Handler {
    fn call(&self, input: Value) -> Result<Value> {
        let operation = required_string(&input, "operation")?;
        let args = input
            .get("args")
            .filter(|value| value.is_object())
            .ok_or_else(|| anyhow!("Missing required object field: args"))?;

        let result = match operation {
            "get" => get_resources(args),
            "describe" => describe_resource(args),
            "logs" => get_logs(args),
            "api-resources" => list_api_resources(args),
            other => Err(anyhow!(
                "Unsupported operation '{}'. Use get, describe, logs, or api-resources.",
                other
            )),
        };

        Ok(match result {
            Ok(data) => json!({ "ok": true, "data": data }),
            Err(error) => json!({ "ok": false, "error": format!("{error:#}") }),
        })
    }
}

fn get_resources(args: &Value) -> Result<Value> {
    let resource = required_positional(args, "resource")?;
    let name = optional_positional(args, "name")?;
    let namespace = optional_string(args, "namespace")?;
    let all_namespaces = optional_bool(args, "all_namespaces")?.unwrap_or(false);

    if all_namespaces && namespace.is_some() {
        return Err(anyhow!(
            "args.all_namespaces cannot be combined with args.namespace"
        ));
    }

    let mut command_args = vec!["get".to_string(), resource.to_string()];
    if let Some(name) = name {
        command_args.push(name.to_string());
    }

    if all_namespaces {
        command_args.push("--all-namespaces".to_string());
    } else if let Some(namespace) = namespace {
        command_args.push(format!("--namespace={namespace}"));
    }

    if let Some(selector) = optional_string(args, "label_selector")? {
        command_args.push(format!("--selector={selector}"));
    }

    if let Some(selector) = optional_string(args, "field_selector")? {
        command_args.push(format!("--field-selector={selector}"));
    }

    command_args.push("--output=json".to_string());
    let stdout = run_kubectl(args, command_args)?;
    serde_json::from_str(&stdout).context("kubectl get returned invalid JSON")
}

fn describe_resource(args: &Value) -> Result<Value> {
    let resource = required_positional(args, "resource")?;
    let name = required_positional(args, "name")?;

    let mut command_args = vec![
        "describe".to_string(),
        resource.to_string(),
        name.to_string(),
    ];
    append_namespace(args, &mut command_args)?;

    let stdout = run_kubectl(args, command_args)?;
    Ok(json!({ "text": stdout }))
}

fn get_logs(args: &Value) -> Result<Value> {
    let pod = required_positional(args, "pod")?;
    let mut command_args = vec!["logs".to_string(), pod.to_string()];
    append_namespace(args, &mut command_args)?;

    if let Some(container) = optional_positional(args, "container")? {
        command_args.push(format!("--container={container}"));
    }

    if let Some(tail_lines) = optional_u64(args, "tail_lines")? {
        if tail_lines == 0 || tail_lines > MAX_LOG_LINES {
            return Err(anyhow!(
                "args.tail_lines must be between 1 and {}",
                MAX_LOG_LINES
            ));
        }
        command_args.push(format!("--tail={tail_lines}"));
    }

    if let Some(since) = optional_string(args, "since")? {
        command_args.push(format!("--since={since}"));
    }

    if optional_bool(args, "previous")?.unwrap_or(false) {
        command_args.push("--previous".to_string());
    }

    let stdout = run_kubectl(args, command_args)?;
    Ok(json!({ "text": stdout }))
}

fn list_api_resources(args: &Value) -> Result<Value> {
    let stdout = run_kubectl(
        args,
        vec![
            "api-resources".to_string(),
            "--output=name".to_string(),
        ],
    )?;

    let resources: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|resource| !resource.is_empty())
        .collect();

    Ok(json!({ "resources": resources }))
}

fn run_kubectl(args: &Value, command_args: Vec<String>) -> Result<String> {
    let mut command = Command::new("kubectl");
    append_connection_options(args, &mut command)?;
    command.args(command_args);

    let output = command
        .output()
        .context("Failed to start kubectl. Ensure kubectl is installed and available on PATH.")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };

        return Err(anyhow!(
            "kubectl exited with status {}: {}",
            output.status,
            detail
        ));
    }

    String::from_utf8(output.stdout).context("kubectl returned non-UTF-8 output")
}

fn append_connection_options(args: &Value, command: &mut Command) -> Result<()> {
    if let Some(kubeconfig) = optional_string(args, "kubeconfig")? {
        command.arg(format!("--kubeconfig={kubeconfig}"));
    }

    if let Some(context) = optional_string(args, "context")? {
        command.arg(format!("--context={context}"));
    }

    Ok(())
}

fn append_namespace(args: &Value, command_args: &mut Vec<String>) -> Result<()> {
    if let Some(namespace) = optional_string(args, "namespace")? {
        command_args.push(format!("--namespace={namespace}"));
    }

    Ok(())
}

fn required_string<'a>(object: &'a Value, field: &str) -> Result<&'a str> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing required string field: {field}"))?;

    if value.trim().is_empty() {
        return Err(anyhow!("Field '{field}' must not be empty"));
    }

    Ok(value)
}

fn optional_string<'a>(object: &'a Value, field: &str) -> Result<Option<&'a str>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => {
            Err(anyhow!("Field '{field}' must not be empty when provided"))
        }
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(anyhow!("Field '{field}' must be a string")),
    }
}

fn required_positional<'a>(object: &'a Value, field: &str) -> Result<&'a str> {
    let value = required_string(object, field)?;
    validate_positional(value, field)?;
    Ok(value)
}

fn optional_positional<'a>(object: &'a Value, field: &str) -> Result<Option<&'a str>> {
    let value = optional_string(object, field)?;
    if let Some(value) = value {
        validate_positional(value, field)?;
    }

    Ok(value)
}

fn validate_positional(value: &str, field: &str) -> Result<()> {
    if value.starts_with('-') {
        return Err(anyhow!(
            "Field '{field}' must not start with '-' because it is a kubectl positional argument"
        ));
    }

    if value.contains('\0') {
        return Err(anyhow!("Field '{field}' must not contain a null byte"));
    }

    Ok(())
}

fn optional_bool(object: &Value, field: &str) -> Result<Option<bool>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(anyhow!("Field '{field}' must be a boolean")),
    }
}

fn optional_u64(object: &Value, field: &str) -> Result<Option<u64>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| anyhow!("Field '{field}' must be a non-negative integer")),
        Some(_) => Err(anyhow!("Field '{field}' must be an integer")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positional_arguments_cannot_be_flags() {
        assert!(validate_positional("--all-namespaces", "resource").is_err());
        assert!(validate_positional("pods", "resource").is_ok());
    }

    #[test]
    fn all_namespaces_cannot_be_combined_with_namespace() {
        let result = get_resources(&json!({
            "resource": "pods",
            "namespace": "default",
            "all_namespaces": true
        }));

        assert!(result.is_err());
    }

    #[test]
    fn log_line_limit_is_enforced_before_kubectl_runs() {
        let result = get_logs(&json!({
            "pod": "api-0",
            "tail_lines": MAX_LOG_LINES + 1
        }));

        assert!(result.is_err());
    }
}