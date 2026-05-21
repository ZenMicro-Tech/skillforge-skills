use anyhow::{anyhow, Result};
use aws_config::SdkConfig;
use serde_json::{json, Value};

pub async fn dispatch(operation: &str, _args: Value, cfg: &SdkConfig) -> Result<Value> {
    let client = aws_sdk_sts::Client::new(cfg);
    match operation {
        "get-caller-identity" => {
            let out = client.get_caller_identity().send().await?;
            Ok(json!({
                "user_id": out.user_id(),
                "account": out.account(),
                "arn": out.arn(),
            }))
        }
        other => Err(anyhow!("sts: unsupported operation {other:?}")),
    }
}
