//! GPU detection and the safety net around it.
//!
//! macOS arm64 builds use Metal, Windows builds use Vulkan, everything else runs on the CPU.
//!
//! On Windows the risks are a missing Vulkan loader (`vulkan-1.dll`) and broken drivers that
//! crash inside Vulkan initialization. The installer ships `vulkan-1.dll` and the exe
//! delay-loads it (see build.rs), so the app always starts. Before touching Vulkan we write a
//! marker file and remove it afterwards; if the app died in between, the next launch finds
//! the marker and switches to CPU for good (the user can turn the GPU back on in Settings).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const MARKER: &str = "gpu-init.marker";

static APP_DATA: OnceLock<PathBuf> = OnceLock::new();

pub fn init(app_data: &Path) {
    let _ = APP_DATA.set(app_data.to_path_buf());
}

fn marker_path() -> Option<PathBuf> {
    APP_DATA.get().map(|d| d.join(MARKER))
}

/// True if the previous run crashed while initializing the GPU. Clears the marker.
pub fn previous_run_crashed_in_gpu_init() -> bool {
    let Some(p) = marker_path() else { return false };
    let crashed = p.is_file();
    if crashed {
        let _ = std::fs::remove_file(&p);
    }
    crashed
}

/// Run `f` with the crash marker in place.
pub fn guarded<T>(f: impl FnOnce() -> T) -> T {
    let marker = marker_path();
    if let Some(p) = &marker {
        let _ = std::fs::write(p, b"1");
    }
    let out = f();
    if let Some(p) = &marker {
        let _ = std::fs::remove_file(p);
    }
    out
}

/// Hide all Vulkan devices from ggml. Must run before the first whisper context is created.
pub fn disable_for_process() {
    if cfg!(feature = "gpu-vulkan") {
        // An empty list makes ggml-vulkan register zero devices, so whisper uses the CPU.
        // SAFETY: called on the main thread during startup, before any other thread reads env.
        unsafe { std::env::set_var("GGML_VK_VISIBLE_DEVICES", "") };
    }
}

/// Whether the Vulkan loader can be loaded at all. Without it, touching whisper would crash
/// on the first delay-loaded Vulkan call, so callers must check this first.
#[cfg(windows)]
pub fn vulkan_loader_present() -> bool {
    use windows_sys::Win32::Foundation::FreeLibrary;
    use windows_sys::Win32::System::LibraryLoader::LoadLibraryW;
    let name: Vec<u16> = "vulkan-1.dll\0".encode_utf16().collect();
    // SAFETY: valid NUL-terminated wide string; the handle is freed right away.
    unsafe {
        let h = LoadLibraryW(name.as_ptr());
        if h.is_null() {
            return false;
        }
        FreeLibrary(h);
    }
    true
}

#[cfg(not(windows))]
pub fn vulkan_loader_present() -> bool {
    true
}

/// (has usable GPU, its name)
pub fn probe() -> (bool, Option<String>) {
    #[cfg(all(feature = "gpu-metal", target_os = "macos", target_arch = "aarch64"))]
    {
        return (true, Some("Apple Silicon (Metal)".into()));
    }
    #[cfg(feature = "gpu-vulkan")]
    {
        if !vulkan_loader_present() {
            return (false, None);
        }
        let devices = guarded(whisper_rs::vulkan::list_devices);
        // Large q5_0 needs ~1.5 GB; integrated GPUs report shared memory, so ask for more.
        return match devices.into_iter().find(|d| d.vram.total >= 3 << 30) {
            Some(d) => (true, Some(d.name)),
            None => (false, None),
        };
    }
    #[allow(unreachable_code)]
    (false, None)
}

/// Whether this build was compiled with any GPU backend.
pub const fn compiled_with_gpu() -> bool {
    cfg!(any(feature = "gpu-metal", feature = "gpu-vulkan"))
}
