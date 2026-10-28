`vulkan-1.dll` (the Khronos Vulkan loader, Apache-2.0) is copied here before a Windows
build and shipped next to `saljtipset.exe`, so the app starts on PCs without a Vulkan
runtime. CI does this automatically; for a local build run `scripts\fetch-vulkan-loader.ps1`.
The DLL itself is not committed.
