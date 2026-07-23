use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for file in ["skill.toml", "prompt.md", "schema.json"] {
        std::fs::copy(file, out.join(file)).unwrap_or_else(|error| panic!("copy {file}: {error}"));
        println!("cargo:rerun-if-changed={file}");
    }
}