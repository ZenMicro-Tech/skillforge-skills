use anyhow::{anyhow, Result};
use aws_config::SdkConfig;
use serde_json::{json, Value};

pub async fn dispatch(operation: &str, _args: Value, cfg: &SdkConfig) -> Result<Value> {
    let client = aws_sdk_iam::Client::new(cfg);
    match operation {
        "list-users" => {
            let out = client.list_users().send().await?;
            let users: Vec<Value> = out
                .users()
                .iter()
                .map(|u| {
                    json!({
                        "user_name": u.user_name(),
                        "user_id": u.user_id(),
                        "arn": u.arn(),
                        "create_date": u.create_date().to_string(),
                    })
                })
                .collect();
            Ok(json!({ "users": users, "is_truncated": out.is_truncated() }))
        }
        "list-roles" => {
            let out = client.list_roles().send().await?;
            let roles: Vec<Value> = out
                .roles()
                .iter()
                .map(|r| {
                    json!({
                        "role_name": r.role_name(),
                        "role_id": r.role_id(),
                        "arn": r.arn(),
                        "create_date": r.create_date().to_string(),
                    })
                })
                .collect();
            Ok(json!({ "roles": roles, "is_truncated": out.is_truncated() }))
        }
        other => Err(anyhow!("iam: unsupported operation {other:?}")),
    }
}
