//! Compila o core C (`crates/higanbana-gpui/core`) e gera os bindings.

use std::path::{Path, PathBuf};

fn main() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../higanbana-gpui/core");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", core.display());

    compile_core(&core);
    generate_bindings(&core, &out);
}

fn c_sources(dir: &Path, acc: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            c_sources(&path, acc);
        } else if path.extension().is_some_and(|e| e == "c") {
            acc.push(path);
        }
    }
}

fn compile_core(core: &Path) {
    let mut sources = Vec::new();
    c_sources(&core.join("src"), &mut sources);
    let mut build = cc::Build::new();
    build
        .files(&sources)
        .include(core.join("include"))
        .include(core.join("src"))
        .include(core.join("src/atlas"))
        .flag_if_supported("/std:c11")
        .warnings(true);
    build.compile("hgui");
}

fn generate_bindings(core: &Path, out: &Path) {
    bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", core.join("include").display()))
        .allowlist_function("hg_.*")
        .allowlist_type("hg_.*")
        .allowlist_var("HG_.*")
        .generate()
        .expect("bindgen falhou em hgui.h")
        .write_to_file(out.join("bindings.rs"))
        .unwrap();
}
