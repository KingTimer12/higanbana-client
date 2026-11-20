//! Memória do próprio processo, para os logs de diagnóstico.

/// Memória privada (a "Memória" do Gerenciador de Tarefas), em MB.
#[cfg(windows)]
pub fn private_mb() -> u64 {
    use windows_sys::Win32::System::{
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX},
        Threading::GetCurrentProcess,
    };
    let mut c: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    // SAFETY: estrutura do tamanho informado em `cb`, processo atual.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS, c.cb) };
    if ok == 0 { 0 } else { c.PrivateUsage as u64 / (1024 * 1024) }
}

#[cfg(not(windows))]
pub fn private_mb() -> u64 {
    0
}
