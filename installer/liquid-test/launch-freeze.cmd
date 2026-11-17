@echo off
rem �ɽ�ͼģʽ������ʱ�����桸���̡���һ�Ÿɾ���ֽ����������������������ų���
rem ������ͼ��¼����������Ļ���ܿ������б������۸к�ʵʱģʽ����һ�£�
rem ����ֻ�Ǳ����������ֽ/����ʵʱ�仯 ���� �㱨����ʱ�������ģʽ��ͼ��
setlocal
set "DAOZAI_DATA_DIR=%LOCALAPPDATA%\Daozai-LiquidTest\data"
set "DAOZAI_DCOMP=1"
set "DAOZAI_LIQUID_FREEZE_BG=1"
cd /d "%~dp0"
start "" "%~dp0daozai.exe"
endlocal
