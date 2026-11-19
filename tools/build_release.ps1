# Build bản phát hành C_Input.exe sạch: không nhúng đường dẫn thư mục của máy build
# (tên người dùng Windows, ổ đĩa, thư mục dự án) vào file chạy.
#
#   powershell -ExecutionPolicy Bypass -File tools\build_release.ps1
#
# Kết quả: dist\release\C_Input.exe + SHA256SUMS.txt

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE ".rustup" }

# Thay các đường dẫn tuyệt đối bằng tên chung chung trong thông báo lỗi nhúng sẵn của thư viện.
$maps = @(
    "--remap-path-prefix=$cargoHome=cargo",
    "--remap-path-prefix=$rustupHome=rustup",
    "--remap-path-prefix=$root=C_Input"
)
# CARGO_ENCODED_RUSTFLAGS tách cờ bằng ký tự 0x1F: đường dẫn có dấu cách vẫn đúng.
$env:CARGO_ENCODED_RUSTFLAGS = ($maps -join [char]0x1F)
cargo build --release -p c_input
if ($LASTEXITCODE -ne 0) { Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue; throw "cargo build thất bại" }
Remove-Item Env:CARGO_ENCODED_RUSTFLAGS

# Kiểm tra lại: không còn tên thư mục người dùng / thư mục dự án trong file chạy.
$exe = Join-Path $root "target\release\C_Input.exe"
$bytes = [System.IO.File]::ReadAllBytes($exe)
$text = [System.Text.Encoding]::GetEncoding(28591).GetString($bytes)
foreach ($p in @($env:USERPROFILE, $root, $env:USERNAME)) {
    if ($p -and $text.Contains($p)) { throw "File chạy vẫn chứa đường dẫn riêng tư: $p" }
}

$out = Join-Path $root "dist\release"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $exe (Join-Path $out "C_Input.exe") -Force
$hash = (Get-FileHash (Join-Path $out "C_Input.exe") -Algorithm SHA256).Hash.ToLower()
Set-Content -Path (Join-Path $out "SHA256SUMS.txt") -Value "$hash  C_Input.exe" -Encoding ASCII
Write-Output "OK: $out\C_Input.exe"
Write-Output "SHA256: $hash"
