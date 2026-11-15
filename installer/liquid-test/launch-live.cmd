@echo off
rem 实时模式：玻璃里是真实的桌面，抓屏实时刷新。
rem 注意：这个模式下岛仔会把自己从系统捕获里排除，任何截图/录屏都拍不到它。
setlocal
set "DAOZAI_DATA_DIR=%LOCALAPPDATA%\Daozai-LiquidTest\data"
cd /d "%~dp0"
start "" "%~dp0daozai.exe"
endlocal
