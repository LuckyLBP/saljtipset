fn main() {
    // Windows + Vulkan: delay-load the Vulkan loader so a machine without it can still start
    // the app (see src/gpu.rs). The installer ships vulkan-1.dll next to the exe as well.
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" && std::env::var_os("CARGO_FEATURE_GPU_VULKAN").is_some() {
        println!("cargo:rustc-link-arg=/DELAYLOAD:vulkan-1.dll");
        println!("cargo:rustc-link-lib=delayimp");
    }

    // macOS: whisper.cpp's Metal code uses `@available(...)`. When building for an older macOS
    // than the build machine (tauri.conf.json sets 12.0), clang turns those into calls to
    // `__isPlatformVersionAtLeast`, which lives in clang's runtime library. rustc links with
    // -nodefaultlibs, so add that library explicitly.
    if target_os == "macos" {
        if let Some(dir) = clang_runtime_dir() {
            println!("cargo:rustc-link-search=native={dir}");
            println!("cargo:rustc-link-lib=static=clang_rt.osx");
        }
    }

    tauri_build::build()
}

fn clang_runtime_dir() -> Option<String> {
    let out = std::process::Command::new("xcrun").args(["clang", "-print-resource-dir"]).output().ok()?;
    let resource = String::from_utf8(out.stdout).ok()?;
    let dir = std::path::Path::new(resource.trim()).join("lib").join("darwin");
    dir.join("libclang_rt.osx.a").is_file().then(|| dir.display().to_string())
}
