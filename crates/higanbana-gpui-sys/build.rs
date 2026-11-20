//! Compila o core C (`crates/higanbana-gpui/core`), os shaders e gera os bindings.

#[path = "build/bindings.rs"]
mod bindings;
#[path = "build/core.rs"]
mod core;
#[path = "build/shaders.rs"]
mod shaders;
#[path = "build/spirv.rs"]
mod spirv;

use std::path::{Path, PathBuf};

fn main() {
    let core_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../higanbana-gpui/core");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", core_dir.display());
    println!("cargo:rerun-if-changed=wrapper.h");

    shaders::compile_all(&core_dir.join("src/shaders"), &out);
    spirv::compile_all(&core_dir.join("src/shaders/vulkan"), &out);
    core::compile(&core_dir, &out);
    bindings::generate(&core_dir, &out);
}
