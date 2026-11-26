//! Opt-in call observation. Original export addresses are retained when off.
use crate::{HleModule, registry::HleExport};
use std::{ffi::c_void, sync::OnceLock};
use windows_sys::Win32::{
    Foundation::{GetLastError, SetLastError},
    System::{Diagnostics::Debug::FlushInstructionCache, Memory::*, Threading::GetCurrentProcess},
};

pub(crate) fn install(modules: &mut [HleModule]) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("VICTORY_TRACE_HLE").is_some_and(|v| v == "1")) {
        return;
    }
    install_calls(modules);
}

fn observe(f: impl FnOnce()) {
    // SAFETY: LastError belongs to this thread; logging must not change it.
    let error = unsafe { GetLastError() };
    f();
    unsafe { SetLastError(error) };
}

extern "system" fn enter(export: &HleExport, arg: *const u16) -> usize {
    observe(|| {
        if matches!(export.name, "CreateFileW" | "GetFileAttributesW") && !arg.is_null() {
            // SAFETY: these exports require a NUL-terminated UTF-16 path.
            let path = unsafe {
                let len = (0..32768).take_while(|&i| *arg.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(arg, len))
            };
            tracing::info!(function = export.name, path, "hle");
        } else {
            tracing::info!(function = export.name, "hle");
        }
    });
    export.address
}

extern "system" fn read_file(
    file: *mut c_void,
    buffer: *mut u8,
    want: u32,
    read: *mut u32,
    overlapped: *mut c_void,
) -> i32 {
    let result = crate::modules::files::ReadFile(file, buffer, want, read, overlapped);
    observe(|| {
        // Only a completed synchronous read guarantees an initialized count.
        let got = if result != 0 && overlapped.is_null() && !read.is_null() {
            // SAFETY: ReadFile has written this output; never inspect buffer.
            Some(unsafe { *read })
        } else {
            None
        };
        if let Some(got) = got {
            tracing::info!(function = "ReadFile", want, got, "hle");
        } else {
            tracing::info!(function = "ReadFile", want, got = "unknown", "hle");
        }
    });
    result
}

// Win64: retain integer/FP arguments and the caller's stack, then tail-call.
#[unsafe(naked)]
unsafe extern "system" fn dispatch() {
    std::arch::naked_asm!(
        "push rcx", "push rdx", "push r8", "push r9", "sub rsp, 104",
        "movdqu [rsp+32], xmm0", "movdqu [rsp+48], xmm1",
        "movdqu [rsp+64], xmm2", "movdqu [rsp+80], xmm3",
        "mov rdx, rcx", "mov rcx, r10", "call {}",
        "movdqu xmm0, [rsp+32]", "movdqu xmm1, [rsp+48]",
        "movdqu xmm2, [rsp+64]", "movdqu xmm3, [rsp+80]",
        "add rsp, 104", "pop r9", "pop r8", "pop rdx", "pop rcx", "jmp rax",
        sym enter,
    );
}

fn install_calls(modules: &mut [HleModule]) {
    let size = modules.iter().map(|m| m.exports.len()).sum::<usize>() * 32;
    if size == 0 {
        return;
    }
    // SAFETY: private RW allocation, made RX before publishing the registry.
    // The code and immutable call sites live as long as the import addresses.
    unsafe {
        let base = VirtualAlloc(
            std::ptr::null(),
            size,
            MEM_RESERVE | MEM_COMMIT,
            PAGE_READWRITE,
        )
        .cast::<u8>();
        assert!(!base.is_null(), "cannot allocate HLE trace entries");
        let mut cursor = base;
        for export in modules.iter_mut().flat_map(|m| &mut m.exports) {
            if export.name == "IID_ID3D12Device" {
                continue;
            } // Data, not code.
            if export.name == "ReadFile" {
                export.address = read_file as *const () as usize;
                continue;
            }
            let site = Box::leak(Box::new(*export));
            let mut thunk = [0xcc; 32];
            thunk[..2].copy_from_slice(&[0x49, 0xba]); // mov r10, site
            thunk[2..10].copy_from_slice(&(site as *const HleExport as u64).to_le_bytes());
            thunk[10..12].copy_from_slice(&[0x48, 0xb8]); // mov rax, dispatch
            thunk[12..20].copy_from_slice(&(dispatch as *const () as u64).to_le_bytes());
            thunk[20..22].copy_from_slice(&[0xff, 0xe0]); // jmp rax
            cursor.copy_from_nonoverlapping(thunk.as_ptr(), thunk.len());
            export.address = cursor as usize;
            cursor = cursor.add(thunk.len());
        }
        let mut old = 0;
        assert_ne!(
            VirtualProtect(base.cast(), size, PAGE_EXECUTE_READ, &mut old),
            0
        );
        assert_ne!(
            FlushInstructionCache(GetCurrentProcess(), base.cast(), size),
            0
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traced_call_preserves_abi_and_last_error() {
        extern "system" fn probe(a: u64, b: f64, c: u64, d: f64, e: u64) -> f64 {
            assert_eq!(unsafe { GetLastError() }, 1234);
            (a + c + e) as f64 + b + d
        }
        let mut modules = [HleModule {
            name: "test",
            dlls: &["test"],
            exports: vec![crate::hle_export!(probe)],
        }];
        install_calls(&mut modules);
        // SAFETY: the trampoline retains probe's ABI.
        let call: extern "system" fn(u64, f64, u64, f64, u64) -> f64 =
            unsafe { std::mem::transmute(modules[0].exports[0].address) };
        unsafe { SetLastError(1234) };
        assert_eq!(call(1, 2.5, 3, 4.5, 5), 16.0);
        assert_eq!(unsafe { GetLastError() }, 1234);
    }
}
