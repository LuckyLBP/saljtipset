# Copies the Vulkan loader (vulkan-1.dll) into src-tauri/windows so the installer ships it.
# Needs the Vulkan SDK or runtime installed (https://vulkan.lunarg.com/sdk/home#windows).
$ErrorActionPreference = "Stop"
$dest = Join-Path $PSScriptRoot "..\src-tauri\windows\vulkan-1.dll"
$candidates = @()
if ($env:VULKAN_SDK) {
  $candidates += Get-ChildItem -Path (Split-Path $env:VULKAN_SDK -Parent) -Recurse -Filter vulkan-1.dll -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match 'x64|Bin' -and $_.FullName -notmatch 'x86|Bin32' }
}
$candidates += Get-Item "$env:WINDIR\System32\vulkan-1.dll" -ErrorAction SilentlyContinue
$src = $candidates | Select-Object -First 1
if (-not $src) { throw "vulkan-1.dll not found. Install the Vulkan SDK first." }
Copy-Item $src.FullName $dest -Force
Write-Host "Copied $($src.FullName) -> $dest"
