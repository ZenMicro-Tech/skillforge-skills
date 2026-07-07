use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for f in ["skill.toml", "prompt.md", "schema.json"] {
        std::fs::copy(f, out.join(f)).unwrap_or_else(|e| panic!("copy {f}: {e}"));
        println!("cargo:rerun-if-changed={f}");
    }
}
