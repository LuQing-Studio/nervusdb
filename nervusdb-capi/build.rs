use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_dir.parent().expect("workspace root").to_path_buf();
    let out_header = workspace_root.join("nervusdb-c-sdk/include/nervusdb.h");

    let config_path = crate_dir.join("cbindgen.toml");
    let config =
        cbindgen::Config::from_file(&config_path).unwrap_or_else(|_| cbindgen::Config::default());

    let generated = cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate();

    if let Ok(header) = generated {
        if let Some(parent) = out_header.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        header.write_to_file(out_header);
    }
}
