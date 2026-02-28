@echo off
chcp 65001 >nul 2>&1
setlocal enabledelayedexpansion
title Cockpit Tools - Build Menu
set "SCRIPT_DIR=%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cd /d "%SCRIPT_DIR%"

:: Support command-line argument for direct execution
if "%1"=="exe"       goto :quick_build
if "%1"=="install"   goto :full_build
if "%1"=="dev"       goto :dev_mode
if "%1"=="frontend"  goto :frontend_only
if "%1"=="rust"      goto :rust_only
if "%1"=="clean"     goto :clean_build

:menu
cls
echo.
echo   +----------------------------------------------+
echo   :       Cockpit Tools - Build Menu v0.9.0       :
echo   +----------------------------------------------+
echo   :                                               :
echo   :   [1]  Quick Build  (exe only, fastest)       :
echo   :   [2]  Full Build   (exe + NSIS installer)    :
echo   :   [3]  Dev Mode     (hot-reload)              :
echo   :   [4]  Frontend Only (vite build)             :
echo   :   [5]  Rust Only    (cargo incremental)       :
echo   :   [6]  Clean Build  (full rebuild)            :
echo   :   [7]  Open Output Folder                     :
echo   :   [8]  Run App                                :
echo   :                                               :
echo   :   [0]  Exit                                   :
echo   :                                               :
echo   +----------------------------------------------+
echo.

set "sel=1"
set /p sel="  >> Press a number to select [Default: 1]: "
if "!sel!"=="0" goto :exit
if "!sel!"=="8" goto :run_app
if "!sel!"=="7" goto :open_folder
if "!sel!"=="6" goto :clean_build
if "!sel!"=="5" goto :rust_only
if "!sel!"=="4" goto :frontend_only
if "!sel!"=="3" goto :dev_mode
if "!sel!"=="2" goto :full_build
if "!sel!"=="1" goto :quick_build
goto :menu

:: ============================================
:: [1] Quick Build - exe only (fastest)
:: ============================================
:quick_build
call :check_env
if !ENV_OK! neq 1 goto :pause_menu
echo.
echo   [Quick Build] Building exe only (via Tauri)...
echo   ------------------------------------------------
echo.
call npm run tauri build -- --no-bundle
if errorlevel 1 goto :fail_rust
echo.
echo   ============================================
echo   [DONE] Build successful!
echo   [EXE]  src-tauri\target\release\cockpit-tools.exe
echo   ============================================
set "OUT_DIR=%SCRIPT_DIR%src-tauri\target\release"
if exist "%OUT_DIR%\cockpit-tools.exe" explorer "%OUT_DIR%"
goto :pause_menu

:: ============================================
:: [2] Full Build - exe + installer
:: ============================================
:full_build
call :check_env
if !ENV_OK! neq 1 goto :pause_menu
echo.
echo   [Full Build] Building exe + NSIS installer...
echo   -----------------------------------------------
echo.
call npm run tauri build -- --bundles nsis
echo.
echo   [DONE] Output:
echo   [EXE]       src-tauri\target\release\cockpit-tools.exe
echo   [Installer]  src-tauri\target\release\bundle\nsis\
goto :pause_menu

:: ============================================
:: [3] Dev Mode
:: ============================================
:dev_mode
call :check_env
if !ENV_OK! neq 1 goto :pause_menu
echo.
echo   [Dev Mode] Starting with hot-reload...
echo   ----------------------------------------
echo   Press Ctrl+C to stop.
echo.
call npm run tauri dev
goto :pause_menu

:: ============================================
:: [4] Frontend Only
:: ============================================
:frontend_only
call :check_env
if !ENV_OK! neq 1 goto :pause_menu
echo.
echo   [Frontend] Building with Vite...
echo   ----------------------------------
echo.
call npm run build
if errorlevel 1 goto :fail_frontend
echo.
echo   [DONE] Frontend built to dist\
goto :pause_menu

:: ============================================
:: [5] Rust Only
:: ============================================
:rust_only
call :check_env
if !ENV_OK! neq 1 goto :pause_menu
echo.
echo   [Rust] Incremental compile...
echo   -------------------------------
echo.
pushd src-tauri
cargo build --release
if errorlevel 1 (
    popd
    goto :fail_rust
)
popd
echo.
echo   [DONE] src-tauri\target\release\cockpit-tools.exe
goto :pause_menu

:: ============================================
:: [6] Clean Build
:: ============================================
:clean_build
call :check_env
if !ENV_OK! neq 1 goto :pause_menu
echo.
echo   [Clean Build] Cleaning all caches...
echo   --------------------------------------
echo.
echo   Cleaning Rust build cache...
pushd src-tauri
cargo clean
popd
if exist "dist" (
    echo   Cleaning frontend dist...
    rd /s /q dist
)
echo   Rebuilding everything...
echo.
call npm run tauri build
echo.
echo   [DONE] Full rebuild complete.
goto :pause_menu

:: ============================================
:: [7] Open Output Folder
:: ============================================
:open_folder
set "OUT_DIR=%SCRIPT_DIR%src-tauri\target\release"
if exist "%OUT_DIR%\cockpit-tools.exe" (
    explorer "%OUT_DIR%"
) else (
    echo.
    echo   [WARN] No build output found. Please build first.
)
goto :pause_menu

:: ============================================
:: [8] Run App
:: ============================================
:run_app
set "EXE_PATH=%SCRIPT_DIR%src-tauri\target\release\cockpit-tools.exe"
if exist "%EXE_PATH%" (
    echo.
    echo   [INFO] Launching Cockpit Tools...
    start "" "%EXE_PATH%"
) else (
    echo.
    echo   [WARN] cockpit-tools.exe not found. Please build first.
)
goto :pause_menu

:: ============================================
:: Utilities
:: ============================================
:check_env
set "ENV_OK=0"
where rustc >nul 2>&1
if errorlevel 1 (
    echo.
    echo   [ERROR] Rust not found. Install from https://rustup.rs
    goto :eof
)
where node >nul 2>&1
if errorlevel 1 (
    echo.
    echo   [ERROR] Node.js not found.
    goto :eof
)
if not exist "node_modules" (
    echo   [INFO] Installing npm dependencies...
    call npm install
)
set "ENV_OK=1"
goto :eof

:fail_frontend
echo.
echo   [ERROR] Frontend build failed!
goto :pause_menu

:fail_rust
echo.
echo   [ERROR] Rust compile failed!
goto :pause_menu

:pause_menu
echo.
echo   -----------------------------------
choice /c YN /n /m "  >> Back to menu? (Y/N): "
if %errorlevel%==2 goto :exit
goto :menu

:exit
echo.
echo   Bye!
endlocal
exit /b 0
