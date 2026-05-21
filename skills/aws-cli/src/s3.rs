use anyhow::{anyhow, Result};
use aws_config::SdkConfig;
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn dispatch(operation: &str, args: Value, cfg: &SdkConfig) -> Result<Value> {
    let client = aws_sdk_s3::Client::new(cfg);
    match operation {
        "list-buckets" => {
            let out = client.list_buckets().send().await?;
            let buckets: Vec<Value> = out
                .buckets()
                .iter()
                .map(|b| {
                    json!({
                        "name": b.name(),
                        "creation_date": b.creation_date().map(|d| d.to_string()),
                    })
                })
                .collect();
            Ok(json!({ "buckets": buckets }))
        }
        "list-objects" => {
            #[derive(Deserialize)]
            struct A {
                bucket: String,
                #[serde(default)]
                prefix: Option<String>,
                #[serde(default)]
                max_keys: Option<i32>,
            }
            let a: A = serde_json::from_value(args)?;
            let mut req = client
                .list_objects_v2()
                .bucket(a.bucket)
                .max_keys(a.max_keys.unwrap_or(100));
            if let Some(p) = a.prefix {
                req = req.prefix(p);
            }
            let out = req.send().await?;
            let objects: Vec<Value> = out
                .contents()
                .iter()
                .map(|o| {
                    json!({
                        "key": o.key(),
                        "size": o.size(),
                        "last_modified": o.last_modified().map(|d| d.to_string()),
                        "storage_class": o.storage_class().map(|s| s.as_str()),
                    })
                })
                .collect();
            Ok(json!({
                "objects": objects,
                "key_count": out.key_count(),
                "is_truncated": out.is_truncated(),
                "next_continuation_token": out.next_continuation_token(),
            }))
        }
        "head-object" => {
            #[derive(Deserialize)]
            struct A {
                bucket: String,
                key: String,
            }
            let a: A = serde_json::from_value(args)?;
            let out = client.head_object().bucket(a.bucket).key(a.key).send().await?;
            Ok(json!({
                "content_length": out.content_length(),
                "content_type": out.content_type(),
                "etag": out.e_tag(),
                "last_modified": out.last_modified().map(|d| d.to_string()),
                "storage_class": out.storage_class().map(|s| s.as_str()),
            }))
        }
        other => Err(anyhow!("s3: unsupported operation {other:?}")),
    }
}
