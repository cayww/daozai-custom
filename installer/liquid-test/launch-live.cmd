@echo off
rem ʵʱģʽ������������ʵ�����棬ץ��ʵʱˢ�¡�
rem ע�⣺���ģʽ�µ��л���Լ���ϵͳ�������ų����κν�ͼ/¼�����Ĳ�������
setlocal
set "DAOZAI_DATA_DIR=%LOCALAPPDATA%\Daozai-LiquidTest\data"
set "DAOZAI_DCOMP=1"
cd /d "%~dp0"
start "" "%~dp0daozai.exe"
endlocal
