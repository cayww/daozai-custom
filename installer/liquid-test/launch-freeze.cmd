@echo off
rem 可截图模式：启动时把桌面「凝固」成一张干净壁纸当玻璃背景，并解除捕获排除，
rem 这样截图、录屏、共享屏幕都能看到岛仔本身。观感和实时模式基本一致，
rem 区别只是背景不再随壁纸/窗口实时变化 —— 汇报问题时请用这个模式截图。
setlocal
set "DAOZAI_DATA_DIR=%LOCALAPPDATA%\Daozai-LiquidTest\data"
set "DAOZAI_LIQUID_FREEZE_BG=1"
cd /d "%~dp0"
start "" "%~dp0daozai.exe"
endlocal
