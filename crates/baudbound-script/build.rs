use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let schemas_dir = manifest_dir.join("../../schemas");
    let program_schema = schemas_dir.join("program.schema.json");
    let nodes_dir = schemas_dir.join("nodes");

    println!("cargo:rerun-if-changed={}", program_schema.display());
    println!("cargo:rerun-if-changed={}", nodes_dir.display());

    let mut resources = fs::read_dir(&nodes_dir)
        .expect("generated node schema directory must exist")
        .map(|entry| entry.expect("node schema directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    resources.sort();

    let generated = format!(
        "pub const PROGRAM_SCHEMA_JSON: &str = include_str!({});\n\
         pub const NODE_SCHEMA_JSONS: &[&str] = &[\n{}];\n",
        rust_path(&program_schema),
        resources
            .iter()
            .map(|path| format!("    include_str!({}),\n", rust_path(path)))
            .collect::<String>()
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"))
        .join("embedded_schemas.rs");
    fs::write(output, generated).expect("embedded schema source must be writable");
}

fn rust_path(path: &Path) -> String {
    format!(
        "r#\"{}\"#",
        path.canonicalize().expect("schema path").display()
    )
}
