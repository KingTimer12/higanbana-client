//! Compila todos os `.c` do core; cada subpasta de `src` vira include dir.

use std::path::{Path, PathBuf};

const SYSTEM_LIBS: &[&str] = &["user32", "dxguid"];

fn walk(dir: &Path, sources: &mut Vec<PathBuf>, dirs: &mut Vec<PathBuf>) {
    dirs.push(dir.to_path_buf());
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, sources, dirs);
        } else if path.extension().is_some_and(|e| e == "c") {
            sources.push(path);
        }
    }
}

pub fn compile(core: &Path, out: &Path) {
    let (mut sources, mut dirs) = (Vec::new(), Vec::new());
    walk(&core.join("src"), &mut sources, &mut dirs);
    let mut build = cc::Build::new();
    build
        .files(&sources)
        .include(core.join("include"))
        .includes(&dirs)
        .include(out) // shaders gerados
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("UNICODE", None)
        .flag_if_supported("/std:c11")
        .warnings(true);
    build.compile("hgui");
    for lib in SYSTEM_LIBS {
        println!("cargo:rustc-link-lib={lib}");
    }
}
