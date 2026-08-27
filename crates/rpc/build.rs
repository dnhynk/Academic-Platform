use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let workspace_dir = manifest_dir.join("../..");
    let schema_root = workspace_dir.join("schemas/proto");
    let schema = schema_root.join("academic/v1/local_core.proto");
    let protoc = protoc_bin_vendored::protoc_bin_path()?;

    println!("cargo:rerun-if-changed={}", schema.display());

    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc);
    config.compile_protos(&[schema], &[schema_root])?;
    Ok(())
}
