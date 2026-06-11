$ErrorActionPreference = 'Continue'

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$cmakeBin = 'C:\Program Files\CMake\bin'
$llvmBin  = 'C:\Program Files\LLVM\bin'
$cudaRoot = 'C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA'

$cudaVer = Get-ChildItem $cudaRoot -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1

$paths = @($cargoBin, $cmakeBin, $llvmBin)

if ($null -ne $cudaVer) {
    $env:CUDA_PATH = $cudaVer.FullName
    $paths += (Join-Path $cudaVer.FullName 'bin\x64')
    $paths += (Join-Path $cudaVer.FullName 'bin')
    $paths += (Join-Path $cudaVer.FullName 'libnvvp')
}

if (Test-Path $llvmBin) { $env:LIBCLANG_PATH = $llvmBin }

foreach ($p in $paths) {
    if ((Test-Path $p) -and ($env:PATH -notlike "*$p*")) {
        $env:PATH = "$p;$env:PATH"
    }
}

Write-Host "build-env ready: CUDA_PATH=$env:CUDA_PATH LIBCLANG_PATH=$env:LIBCLANG_PATH"
