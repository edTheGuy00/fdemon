@echo off
REM tests/docker/windows/oem/install.bat
REM
REM Auto-run by dockur/windows at the end of the unattended Windows install
REM (the /oem folder is copied to C:\OEM and install.bat is executed once).
REM
REM This PRE-STAGES the test environment so that when you RDP in, everything is
REM ready: fdemon.exe on PATH + a minimal runnable Flutter project + a baseline
REM smoke log. It does NOT perform the registry round-trip — that is exercised
REM interactively by the wizard as the logged-in user (see README).
REM
REM NOTE: install.bat runs in the SYSTEM context during setup, so a registry
REM snapshot here would read the wrong hive. Inspect HKCU:\Environment AFTER you
REM run the wizard as the Docker user instead.

REM 1. Install fdemon.exe and put it on the machine PATH.
mkdir C:\fdemon 2>nul
copy /Y C:\OEM\fdemon.exe C:\fdemon\fdemon.exe
copy /Y C:\OEM\reset-toolchain.ps1 C:\fdemon\reset-toolchain.ps1 2>nul
setx /M PATH "%PATH%;C:\fdemon" >nul 2>&1

REM 2. Create a minimal runnable Flutter project. On Windows, a runnable project
REM    needs a pubspec.yaml with a flutter SDK dep AND a platform dir (windows\).
mkdir C:\test-project\windows 2>nul
> C:\test-project\pubspec.yaml echo name: test_project
>> C:\test-project\pubspec.yaml echo description: Toolchain bootstrap E2E test project
>> C:\test-project\pubspec.yaml echo dependencies:
>> C:\test-project\pubspec.yaml echo   flutter:
>> C:\test-project\pubspec.yaml echo     sdk: flutter
>> C:\test-project\pubspec.yaml echo environment:
>> C:\test-project\pubspec.yaml echo   sdk: ">=3.0.0 <4.0.0"

REM 3. Non-interactive smoke: run `fdemon doctor` and capture output to the
REM    Shared folder so the result is visible from the host (./shared on Linux).
C:\fdemon\fdemon.exe doctor C:\test-project > C:\OEM\doctor-output.txt 2>&1
copy /Y C:\OEM\doctor-output.txt "%PUBLIC%\Desktop\Shared\doctor-output.txt" 2>nul
