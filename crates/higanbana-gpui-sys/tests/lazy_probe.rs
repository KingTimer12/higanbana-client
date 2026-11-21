//! Binário de teste próprio (processo limpo): um app em D3D11 que lista os backends
//! não pode carregar drivers dos outros. Antes, o probe de todos custava ~75 MB.
mod common;
use common::*;
use higanbana_gpui_sys::*;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> isize;
    fn K32EnumProcessModules(p: isize, m: *mut isize, cb: u32, needed: *mut u32) -> i32;
    fn K32GetModuleBaseNameW(p: isize, m: isize, name: *mut u16, n: u32) -> u32;
}

fn modules() -> Vec<String> {
    let mut m = vec![0isize; 2048];
    let mut need = 0u32;
    unsafe { K32EnumProcessModules(GetCurrentProcess(), m.as_mut_ptr(), (m.len() * 8) as u32, &mut need) };
    m[..need as usize / 8].iter().map(|&h| {
        let mut buf = [0u16; 260];
        let n = unsafe { K32GetModuleBaseNameW(GetCurrentProcess(), h, buf.as_mut_ptr(), 260) };
        String::from_utf16_lossy(&buf[..n as usize]).to_lowercase()
    }).collect()
}

#[test]
fn d3d11_app_enumerating_backends_loads_no_other_driver() {
    #[cfg(feature = "wgpu")]
    assert!(wgpu_backend::install());
    let f = Fixture::new();
    assert_eq!(f.frame(), [OK; 3]);
    let before = modules();

    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    let kinds: Vec<_> = infos[..n].iter().map(|i| i.kind).collect();

    let new: Vec<_> = modules().into_iter().filter(|m| !before.contains(m)).collect();
    assert!(new.is_empty(), "enumerar deixou módulos carregados: {new:?}");
    assert!(!before.contains(&"vulkan-1.dll".to_string()), "app D3D11 carregou o loader Vulkan");

    assert!(kinds.contains(&hg_backend_kind_HG_BACKEND_D3D11));
    assert!(kinds.contains(&hg_backend_kind_HG_BACKEND_OPENGL));
    assert!(infos[..n].iter().all(|i| i.adapter[0] != 0), "toda entrada traz o nome da GPU");
    let vk = std::env::var("SystemRoot").unwrap() + r"\System32\vulkan-1.dll";
    if std::path::Path::new(&vk).exists() {
        assert!(kinds.contains(&hg_backend_kind_HG_BACKEND_VULKAN));
    }
}
