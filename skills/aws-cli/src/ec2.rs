use anyhow::{anyhow, Result};
use aws_config::{BehaviorVersion, Region, SdkConfig};
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn dispatch(operation: &str, args: Value, cfg: &SdkConfig) -> Result<Value> {
    match operation {
        "describe-instances" => {
            #[derive(Deserialize, Default)]
            struct A {
                #[serde(default)]
                region: Option<String>,
            }
            let a: A = serde_json::from_value(args).unwrap_or_default();
            let cfg = override_region(cfg, a.region).await;
            let client = aws_sdk_ec2::Client::new(&cfg);
            let out = client.describe_instances().send().await?;
            let reservations: Vec<Value> = out
                .reservations()
                .iter()
                .map(|r| {
                    let instances: Vec<Value> = r
                        .instances()
                        .iter()
                        .map(|i| {
                            json!({
                                "instance_id": i.instance_id(),
                                "instance_type": i.instance_type().map(|t| t.as_str()),
                                "state": i.state().and_then(|s| s.name()).map(|n| n.as_str()),
                                "private_ip": i.private_ip_address(),
                                "public_ip": i.public_ip_address(),
                                "launch_time": i.launch_time().map(|d| d.to_string()),
                                "tags": i.tags().iter().map(|t| json!({
                                    "key": t.key(),
                                    "value": t.value(),
                                })).collect::<Vec<_>>(),
                            })
                        })
                        .collect();
                    json!({ "reservation_id": r.reservation_id(), "instances": instances })
                })
                .collect();
            Ok(json!({ "reservations": reservations }))
        }
        "describe-regions" => {
            let client = aws_sdk_ec2::Client::new(cfg);
            let out = client.describe_regions().send().await?;
            let regions: Vec<Value> = out
                .regions()
                .iter()
                .map(|r| {
                    json!({
                        "name": r.region_name(),
                        "endpoint": r.endpoint(),
                        "opt_in_status": r.opt_in_status(),
                    })
                })
                .collect();
            Ok(json!({ "regions": regions }))
        }
        other => Err(anyhow!("ec2: unsupported operation {other:?}")),
    }
}

async fn override_region(base: &SdkConfig, region: Option<String>) -> SdkConfig {
    match region {
        Some(r) => {
            aws_config::defaults(BehaviorVersion::latest())
                .region(Region::new(r))
                .load()
                .await
        }
        None => base.clone(),
    }
}
