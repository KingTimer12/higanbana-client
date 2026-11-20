//! Compila WGSL (`src/shaders/vulkan/*.wgsl`) para SPIR-V com o naga e gera
//! headers C (`<nome>_<estágio>_spv.h`, array `hg_<nome>_<estágio>_spv[]`) em OUT_DIR.

use naga::back::spv;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::fmt::Write;
use std::path::Path;

const STAGES: &[(&str, &str, naga::ShaderStage)] =
    &[("vs", "vs_main", naga::ShaderStage::Vertex), ("fs", "fs_main", naga::ShaderStage::Fragment)];

fn to_header(var: &str, words: &[u32]) -> String {
    let mut s = format!("/* gerado por build/spirv.rs */\n#include <stdint.h>\nstatic const uint32_t {var}[] = {{");
    for (i, w) in words.iter().enumerate() {
        if i % 8 == 0 {
            s.push_str("\n   ");
        }
        write!(s, " 0x{w:08x},").unwrap();
    }
    s.push_str("\n};\n");
    s
}

fn compile(path: &Path, out: &Path) {
    let name = path.file_stem().unwrap().to_string_lossy().into_owned();
    let src = std::fs::read_to_string(path).unwrap();
    let module = naga::front::wgsl::parse_str(&src)
        .unwrap_or_else(|e| panic!("{}: {}", path.display(), e.emit_to_string(&src)));
    let info = Validator::new(ValidationFlags::all(), Capabilities::IMMEDIATES)
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}: validação falhou: {e:?}", path.display()));
    let options = spv::Options::default();
    for (suffix, entry, stage) in STAGES {
        let pipeline = spv::PipelineOptions { shader_stage: *stage, entry_point: (*entry).into() };
        let words = spv::write_vec(&module, &info, &options, Some(&pipeline))
            .unwrap_or_else(|e| panic!("{} ({entry}): {e}", path.display()));
        let header = to_header(&format!("hg_{name}_{suffix}_spv"), &words);
        std::fs::write(out.join(format!("{name}_{suffix}_spv.h")), header).unwrap();
    }
}

pub fn compile_all(dir: &Path, out: &Path) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "wgsl") {
            compile(&path, out);
        }
    }
}
