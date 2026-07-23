use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let schemas_dir = manifest_dir.join("../../contracts");
    if !schemas_dir.join("contract.json").is_file() {
        panic!(
            "contracts submodule is not initialized; run `git submodule update --init --recursive`"
        );
    }
    let manifest_schema = schemas_dir.join("manifest.schema.json");
    let program_schema = schemas_dir.join("program.schema.json");
    let repository_schema = schemas_dir.join("repository.schema.json");
    let node_capabilities = schemas_dir.join("runner/node-capabilities.json");
    let node_permissions = schemas_dir.join("runner/node-permissions.json");
    let nodes_dir = schemas_dir.join("nodes");

    println!("cargo:rerun-if-changed={}", manifest_schema.display());
    println!("cargo:rerun-if-changed={}", program_schema.display());
    println!("cargo:rerun-if-changed={}", repository_schema.display());
    println!("cargo:rerun-if-changed={}", node_capabilities.display());
    println!("cargo:rerun-if-changed={}", node_permissions.display());
    println!("cargo:rerun-if-changed={}", nodes_dir.display());

    let capabilities = contract_capabilities(&node_capabilities);
    let permissions = contract_permissions(&node_permissions);

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
        "pub const MANIFEST_SCHEMA_JSON: &str = include_str!({});\n\
         pub const PROGRAM_SCHEMA_JSON: &str = include_str!({});\n\
         pub const REPOSITORY_SCHEMA_JSON: &str = include_str!({});\n\
         pub const REPOSITORY_CAPABILITY_NAMES: &[&str] = &[\n{}];\n\
         pub const REPOSITORY_PERMISSION_NAMES: &[&str] = &[\n{}];\n\
         pub const NODE_SCHEMA_JSONS: &[&str] = &[\n{}];\n",
        rust_path(&manifest_schema),
        rust_path(&program_schema),
        rust_path(&repository_schema),
        render_string_slice(&capabilities),
        render_string_slice(&permissions),
        resources
            .iter()
            .map(|path| format!("    include_str!({}),\n", rust_path(path)))
            .collect::<String>()
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"))
        .join("embedded_schemas.rs");
    fs::write(output, generated).expect("embedded schema source must be writable");
}

fn contract_capabilities(path: &Path) -> BTreeSet<String> {
    let contract = read_json(path);
    contract["nodes"]
        .as_object()
        .expect("node capabilities contract must contain a nodes object")
        .values()
        .flat_map(|value| {
            value
                .as_array()
                .expect("node capabilities must be arrays")
                .iter()
        })
        .map(|value| {
            value
                .as_str()
                .expect("node capabilities must be strings")
                .to_owned()
        })
        .collect()
}

fn contract_permissions(path: &Path) -> BTreeSet<String> {
    let contract = read_json(path);
    contract["nodes"]
        .as_object()
        .expect("node permissions contract must contain a nodes object")
        .values()
        .filter_map(|value| value.get("permission"))
        .filter_map(|value| value.get("name"))
        .map(|value| {
            value
                .as_str()
                .expect("permission names must be strings")
                .to_owned()
        })
        .collect()
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn render_string_slice(values: &BTreeSet<String>) -> String {
    values
        .iter()
        .map(|value| format!("    {value:?},\n"))
        .collect()
}

fn rust_path(path: &Path) -> String {
    format!(
        "r#\"{}\"#",
        path.canonicalize().expect("schema path").display()
    )
}
