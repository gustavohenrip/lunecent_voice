@echo off
setlocal enabledelayedexpansion

echo ============================================
echo  Lunecent Voice - Windows Setup
echo ============================================
echo.

where winget >nul 2>&1
if errorlevel 1 (
    echo ERRO: winget nao encontrado. Instale o "App Installer" pela Microsoft Store e rode novamente.
    exit /b 1
)

set "CUDA_CHOICE="
echo Quer instalar o CUDA Toolkit? ^(necessario apenas para build com GPU NVIDIA, ~3GB^)
echo   1 - yes
echo   2 - no
set /p CUDA_CHOICE="Escolha [1/2]: "
echo.

echo [1/6] Visual Studio 2022 Build Tools (MSVC C++ + Windows SDK)...
winget list --id Microsoft.VisualStudio.2022.BuildTools -e >nul 2>&1
if errorlevel 1 (
    winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements --accept-package-agreements --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
) else (
    echo     ja instalado, pulando.
)

echo [2/6] Rust (rustup)...
winget list --id Rustlang.Rustup -e >nul 2>&1
if errorlevel 1 (
    winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
) else (
    echo     ja instalado, pulando.
)
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
rustup default stable-msvc >nul 2>&1

echo [3/6] LLVM (libclang)...
winget list --id LLVM.LLVM -e >nul 2>&1
if errorlevel 1 (
    winget install --id LLVM.LLVM -e --accept-source-agreements --accept-package-agreements
) else (
    echo     ja instalado, pulando.
)

echo [4/6] CMake...
winget list --id Kitware.CMake -e >nul 2>&1
if errorlevel 1 (
    winget install --id Kitware.CMake -e --accept-source-agreements --accept-package-agreements
) else (
    echo     ja instalado, pulando.
)

echo [5/6] Node.js LTS...
winget list --id OpenJS.NodeJS.LTS -e >nul 2>&1
if errorlevel 1 (
    winget install --id OpenJS.NodeJS.LTS -e --accept-source-agreements --accept-package-agreements
) else (
    echo     ja instalado, pulando.
)

echo [6/6] CUDA Toolkit...
if "%CUDA_CHOICE%"=="1" (
    winget list --id Nvidia.CUDA -e >nul 2>&1
    if errorlevel 1 (
        winget install --id Nvidia.CUDA -e --accept-source-agreements --accept-package-agreements
    ) else (
        echo     ja instalado, pulando.
    )
) else (
    echo     pulado por escolha do usuario.
)

echo.
echo Instalando dependencias do frontend (npm install)...
set "PATH=%ProgramFiles%\nodejs;%PATH%"
pushd "%~dp0..\.."
where npm >nul 2>&1
if errorlevel 1 (
    echo AVISO: npm ainda nao esta no PATH desta janela. Abra um novo terminal e rode: npm install
) else (
    call npm install
)
popd

echo.
echo ============================================
echo  Setup concluido.
echo  Feche e reabra o terminal para o PATH atualizar.
echo  Para rodar em modo dev: scripts\windows\run-dev.bat
echo ============================================
endlocal
