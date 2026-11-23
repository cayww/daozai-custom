@echo off
setlocal

rem Frame-timing only capture. Edit the three values below.
set "PRESENTMON_EXE=C:\Tools\PresentMon\PresentMon.exe"
set "TARGET_PID=0"
set "SECONDS=60"
set "OUT=%~dp0..\perf-captures\presentmon.csv"

if not exist "%PRESENTMON_EXE%" (
  echo [ERROR] PresentMon.exe not found: %PRESENTMON_EXE%
  echo Download from https://github.com/GameTechDev/PresentMon/releases and update PRESENTMON_EXE.
  exit /b 1
)

if "%TARGET_PID%"=="0" (
  rem Capture by process name; captures all daozai.exe instances if several exist.
  "%PRESENTMON_EXE%" --process_name daozai --timed %SECONDS% --terminate_after_timed --date_time --write_display_metadata --no_console_stats --output_file "%OUT%"
) else (
  rem Capture by PID; use this when aligning with collect-wpr.ps1 ETL.
  "%PRESENTMON_EXE%" --process_id %TARGET_PID% --timed %SECONDS% --terminate_after_timed --date_time --write_display_metadata --no_console_stats --output_file "%OUT%"
)

echo CSV: %OUT%
endlocal
