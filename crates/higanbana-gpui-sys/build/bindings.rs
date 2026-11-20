//! Gera `bindings.rs` a partir de `hgui.h` + `hgui_internal.h` (via wrapper.h).

use std::path::Path;

pub fn generate(core: &Path, out: &Path) {
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
