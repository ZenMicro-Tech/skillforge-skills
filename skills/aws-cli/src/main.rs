use anyhow::{anyhow, Context, Result};
use aws_config::SdkConfig;
use serde::Deserialize;
use serde_json::{json, Value};
use skillforge_runtime::{dispatch, Embedded, SkillHandler};

mod ec2;
mod iam;
mod s3;
mod sts;

#[derive(Debug, Deserialize)]
struct Call {
    service: String,
    operation: String,
    #[serde(default)]
    args: Value,
}

struct Handler;

impl SkillHandler for Handler {
    fn call(&self, input: Value) -> Result<Value> {
        let call: Call = serde_json::from_value(input).context("invalid input")?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("building tokio runtime")?;

        let result = rt.block_on(async move {
            let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            run(&call.service, &call.operation, call.args, &cfg).await
        });

        Ok(match result {
            Ok(data) => json!({ "ok": true, "data": data }),
            Err(e) => json!({ "ok": false, "error": format!("{e:#}") }),
        })
    }
}

async fn run(service: &str, operation: &str, args: Value, cfg: &SdkConfig) -> Result<Value> {
    match service {
        "s3" => s3::dispatch(operation, args, cfg).await,
        "ec2" => ec2::dispatch(operation, args, cfg).await,
        "sts" => sts::dispatch(operation, args, cfg).await,
        "iam" => iam::dispatch(operation, args, cfg).await,
        other => Err(anyhow!("unknown service {other:?}")),
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
