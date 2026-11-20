//! Gera `bindings.rs` a partir de `hgui.h`, `hgui_internal.h`, `hgui_backend.h` e
//! `backend.h` (vtable para backends externos), via wrapper.h.

use std::path::{Path, PathBuf};

/// bindgen precisa de `libclang.dll`. Se `LIBCLANG_PATH` não está definido, procura
/// no PATH e em instalações comuns do LLVM, para `cargo build` funcionar sem setup.
fn locate_libclang() {
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");
    if std::env::var_os("LIBCLANG_PATH").is_some() {
        return;
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let common = [r"C:Program Fileslvmbin", r"c:program files (x86)lvmbin"];
    let found = std::env::split_paths(&path)
        .chain(common.iter().map(PathBuf::from))
        .find(|dir| dir.join("libclang.dll").is_file() || dir.join("clang.dll").is_file());
    match found {
        // SAFETY: build script de thread única; bindgen lê a variável logo em seguida.
        Some(dir) => unsafe { std::env::set_var("LIBCLANG_PATH", dir) },
        None => panic!("libclang.dll não encontrado: instale o LLVM ou defina LIBCLANG_PATH"),
    }
}

pub fn generate(core: &Path, out: &Path) {
    locate_libclang();
    bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", core.join("include").display()))
        .clang_arg(format!("-I{}", core.join("src").display()))
        .allowlist_function("hg_.*")
        .allowlist_type("hg_.*")
        .allowlist_var("HG_.*")
        .generate()
        .expect("bindgen falhou em hgui.h")
        .write_to_file(out.join("bindings.rs"))
        .unwrap();
}
