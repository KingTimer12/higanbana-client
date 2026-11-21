//! Compila HLSL com `fxc` para headers C (`<nome>_vs.h`/`<nome>_ps.h`) em OUT_DIR.
//! Variável `HG_FXC` sobrescreve o caminho; senão busca o Windows SDK mais novo.

use std::path::{Path, PathBuf};
use std::process::Command;

const STAGES: &[(&str, &str, &str)] = &[("vs", "vs_main", "vs_5_0"), ("ps", "ps_main", "ps_5_0")];

fn find_fxc() -> PathBuf {
    if let Ok(p) = std::env::var("HG_FXC") {
        return PathBuf::from(p);
    }
    let kits = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(kits)
        .expect("Windows SDK não encontrado; defina HG_FXC")
        .flatten()
        .map(|e| e.path().join("x64").join("fxc.exe"))
        .filter(|p| p.exists())
        .collect();
    versions.sort();
    versions.pop().expect("fxc.exe não encontrado; defina HG_FXC")
}

pub fn compile_all(dir: &Path, out: &Path) {
    println!("cargo:rerun-if-env-changed=HG_FXC");
    let fxc = find_fxc();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "hlsl") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        for (suffix, entry_point, profile) in STAGES {
            let var = format!("hg_{name}_{suffix}");
            let header = out.join(format!("{name}_{suffix}.h"));
            let status = Command::new(&fxc)
                .args(["/nologo", "/O3", "/T", profile, "/E", entry_point, "/Vn", &var, "/Fh"])
                .arg(&header)
                .arg(&path)
                .status()
                .expect("falha ao executar fxc");
            assert!(status.success(), "fxc falhou em {} ({entry_point})", path.display());
        }
    }
}
