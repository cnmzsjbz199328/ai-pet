# build.ps1 — 在普通 PowerShell 中加载 MSVC 环境后执行 cargo build
# 用法：在项目根目录运行  .\build.ps1
# 可选参数：.\build.ps1 -Release  （编译 release 版本）

param([switch]$Release)

$logFile = "C:\Users\tj169\build_output.txt"
"=== Build started $(Get-Date) ===" | Out-File $logFile -Encoding utf8

# 用 vswhere 查找 vcvars64.bat
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) {
    $vswhere = "${env:ProgramFiles}\Microsoft Visual Studio\Installer\vswhere.exe"
}

$vsPath = $null
if (Test-Path $vswhere) {
    "vswhere found at: $vswhere" | Out-File $logFile -Append -Encoding utf8
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>&1
    "vswhere result: $vsPath" | Out-File $logFile -Append -Encoding utf8
}

# 候选 vcvars64.bat 路径列表
$candidates = @()
if ($vsPath) {
    $candidates += "$vsPath\VC\Auxiliary\Build\vcvars64.bat"
}
$candidates += @(
    "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
)

$vcvars = $null
foreach ($c in $candidates) {
    if (Test-Path $c) {
        $vcvars = $c
        "Found vcvars64.bat at: $c" | Out-File $logFile -Append -Encoding utf8
        break
    }
    "Not found: $c" | Out-File $logFile -Append -Encoding utf8
}

if (-not $vcvars) {
    $msg = "ERROR: Cannot find vcvars64.bat. Install Visual Studio 2022 with C++ workload or VS Build Tools."
    $msg | Out-File $logFile -Append -Encoding utf8
    Write-Error $msg
    exit 1
}

# 从 vcvars64.bat 中提取环境变量并注入当前 PowerShell session
"Loading MSVC environment from: $vcvars" | Out-File $logFile -Append -Encoding utf8
$envOutput = cmd /c "`"$vcvars`" > nul 2>&1 && set"
foreach ($line in $envOutput) {
    if ($line -match '^([^=]+)=(.*)$') {
        [System.Environment]::SetEnvironmentVariable($Matches[1], $Matches[2])
    }
}

# 确保 cargo 在 PATH 中（scoop 安装的 rustup 路径）
$cargoBin = "$env:USERPROFILE\scoop\persist\rustup\.cargo\bin"
if (Test-Path $cargoBin) {
    $env:PATH = "$cargoBin;$env:PATH"
}

Write-Host "MSVC environment loaded." -ForegroundColor Green
"MSVC environment loaded. cargo=$(where.exe cargo 2>$null | Select-Object -First 1)" | Out-File $logFile -Append -Encoding utf8

# 运行 cargo build
Set-Location $PSScriptRoot
$cargoCmd = if ($Release) { "cargo build --release" } else { "cargo build" }
"Running: $cargoCmd" | Out-File $logFile -Append -Encoding utf8

cmd /c "$cargoCmd >> `"$logFile`" 2>&1"
$exitCode = $LASTEXITCODE
"=== Build finished with exit code $exitCode ===" | Out-File $logFile -Append -Encoding utf8
exit $exitCode
