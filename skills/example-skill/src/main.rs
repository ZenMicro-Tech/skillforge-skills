use anyhow::Result;
use serde_json::{json, Value};
use skillforge_runtime::{dispatch, Embedded, SkillHandler};

struct Handler;

impl SkillHandler for Handler {
    fn call(&self, input: Value) -> Result<Value> {
        let text = input.get("input").and_then(Value::as_str).unwrap_or("");
        Ok(json!({ "echo": text }))
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
