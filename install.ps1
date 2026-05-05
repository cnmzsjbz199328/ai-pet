# install.ps1 — 构建 release 版本并安装到 %USERPROFILE%\.ai-pet\
# 用法（在项目根目录下运行）：
#   .\install.ps1
# 安装完成后可在任意终端执行：ai-pet start / play / stop

param()

$ErrorActionPreference = "Stop"
$installDir = "$env:USERPROFILE\.ai-pet"
$binDir     = "$installDir\bin"

# ---------------------------------------------------------------------------
# 1. 加载 MSVC 环境（与 build.ps1 相同逻辑）
# ---------------------------------------------------------------------------
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) {
    $vswhere = "${env:ProgramFiles}\Microsoft Visual Studio\Installer\vswhere.exe"
}

$candidates = @(
    "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
)
if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    if ($vsPath) { $candidates = @("$vsPath\VC\Auxiliary\Build\vcvars64.bat") + $candidates }
}

$vcvars = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $vcvars) {
    Write-Error "Cannot find vcvars64.bat. Install Visual Studio 2022 Build Tools with C++ workload."
    exit 1
}
$envOutput = cmd /c "`"$vcvars`" > nul 2>&1 && set"
foreach ($line in $envOutput) {
    if ($line -match '^([^=]+)=(.*)$') {
        [System.Environment]::SetEnvironmentVariable($Matches[1], $Matches[2])
    }
}

$cargoBin = "$env:USERPROFILE\scoop\persist\rustup\.cargo\bin"
if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }

# ---------------------------------------------------------------------------
# 2. 构建 release
# ---------------------------------------------------------------------------
Write-Host "Building release..." -ForegroundColor Cyan
Set-Location $PSScriptRoot
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Error "Build failed"; exit 1 }

# ---------------------------------------------------------------------------
# 3. 复制文件到安装目录
# ---------------------------------------------------------------------------
Write-Host "Installing to $installDir ..." -ForegroundColor Cyan

New-Item -ItemType Directory -Force -Path $binDir | Out-Null
# target-dir 在 .cargo/config.toml 中被重定向到 C:\cargo-build\ai-pet
$exePath = "C:\cargo-build\ai-pet\release\ai-pet.exe"
if (-not (Test-Path $exePath)) {
    $exePath = "target\release\ai-pet.exe"  # fallback
}
Copy-Item $exePath "$binDir\ai-pet.exe" -Force

# 复制 assets 到 bin/ 同级（exe 启动时在 exe 所在目录查找 assets/）
if (Test-Path "$binDir\assets") {
    Remove-Item "$binDir\assets" -Recurse -Force
}
Copy-Item "assets" "$binDir\assets" -Recurse -Force

# 复制 .env 到 bin/（dotenvy 从 cwd 读取，而 cwd = exe 所在目录）
if (Test-Path ".env") {
    Copy-Item ".env" "$binDir\.env" -Force
    Write-Host "Copied .env to $binDir\.env" -ForegroundColor Yellow
}

# ---------------------------------------------------------------------------
# 4. 将 bin 目录加入用户 PATH（若尚未加入）
# ---------------------------------------------------------------------------
$userPath = [System.Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPath -notlike "*$binDir*") {
    [System.Environment]::SetEnvironmentVariable("PATH", "$binDir;$userPath", "User")
    Write-Host "Added $binDir to user PATH." -ForegroundColor Green
    Write-Host "Please restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
} else {
    Write-Host "$binDir already in PATH." -ForegroundColor Green
}

Write-Host ""
Write-Host "Installation complete!" -ForegroundColor Green
Write-Host "Usage (from any directory, after restarting terminal):" -ForegroundColor White
Write-Host "  ai-pet start" -ForegroundColor Cyan
Write-Host "  ai-pet play `"让猫咪走来走去`"" -ForegroundColor Cyan
Write-Host "  ai-pet stop" -ForegroundColor Cyan
Write-Host ""
Write-Host "Log: $installDir\daemon.log" -ForegroundColor Gray
Write-Host "API key: edit $installDir\.env" -ForegroundColor Gray
