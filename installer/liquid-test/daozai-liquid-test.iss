; 岛仔「液态玻璃」测试版安装包 —— Inno Setup 7
; 编译：ISCC.exe daozai-liquid-test.iss
;
; 和正式版 daozai.iss 刻意做成两个互不干扰的安装：
;   · 独立 AppId / 独立安装目录 / 独立用户数据目录
;   · 不注册浏览器扩展桥（测试只关心玻璃观感）
; 这样测试者即使已经装了正式版岛仔，也不会被这个包改掉皮肤和设置。

#define MyAppName "岛仔 液态玻璃测试版"
#define MyAppVersion "0.6.64"
#define MyAppBuild "liquid-test-12"
; 独立数据目录：不和正式版的 %APPDATA%\Daozai 抢设置
#define MyDataDir "Daozai-LiquidTest"

[Setup]
; 全新 GUID，与正式版 {{A58C1D9E-...}} 完全不同 —— 否则会互相覆盖/卸载对方
AppId={{C7E41B92-6D38-4F15-8A2E-93B5F0D61C27}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}（{#MyAppBuild}）
AppPublisher=自定义构建（测试用）
; {autopf} 在 lowest 权限下解析为 %LOCALAPPDATA%\Programs，普通用户可写、无需管理员
DefaultDirName={autopf}\DaozaiLiquidTest
DisableProgramGroupPage=yes
DefaultGroupName=岛仔液态玻璃测试
PrivilegesRequired=lowest
OutputDir=.
OutputBaseFilename=daozai-{#MyAppBuild}
SetupIconFile=..\..\dz-phone\assets\daozai.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
UninstallDisplayName={#MyAppName}（{#MyAppBuild}）
UninstallDisplayIcon={app}\daozai.ico

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "其他："; Flags: checkedonce

[Files]
Source: "..\..\dz-phone\target\release\daozai.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dz-phone\assets\daozai.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "launch-live.cmd"; DestDir: "{app}"; Flags: ignoreversion
Source: "launch-freeze.cmd"; DestDir: "{app}"; Flags: ignoreversion
Source: "测试说明.txt"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Icons]
Name: "{group}\岛仔 液态玻璃（实时）"; Filename: "{app}\launch-live.cmd"; IconFilename: "{app}\daozai.ico"
Name: "{group}\岛仔 液态玻璃（可截图）"; Filename: "{app}\launch-freeze.cmd"; IconFilename: "{app}\daozai.ico"
Name: "{group}\测试说明"; Filename: "{app}\测试说明.txt"
Name: "{autodesktop}\岛仔 液态玻璃"; Filename: "{app}\launch-live.cmd"; IconFilename: "{app}\daozai.ico"; Tasks: desktopicon

[UninstallDelete]
Type: files; Name: "{app}\CUSTOM_BUILD.txt"

[Run]
Filename: "{app}\测试说明.txt"; Description: "先看测试说明（截图注意事项在里面）"; Flags: shellexec nowait postinstall skipifsilent

[Code]
// 首次安装时铺一份「液态玻璃」默认设置。
// 数据目录是独立的（%LOCALAPPDATA%\Daozai-LiquidTest\data），所以只在自己这份里写，
// 不会碰到正式版的 %APPDATA%\Daozai。
procedure SeedSettings;
var
  dir, file: string;
begin
  dir := ExpandConstant('{localappdata}\{#MyDataDir}\data');
  ForceDirectories(dir);
  file := dir + '\settings.txt';
  // 已存在就绝不覆盖：测试者调过的参数不能被升级安装重置
  if FileExists(file) then exit;
  SaveStringToFile(file,
    'skin=liquid_glass' + #13#10 +
    'render_compat=1' + #13#10 +
    'island_shape=pill' + #13#10 +
    'dock=top' + #13#10 +
    'island_size=120' + #13#10 +
    'update_check=0' + #13#10,
    False);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    SeedSettings;
    SaveStringToFile(ExpandConstant('{app}\CUSTOM_BUILD.txt'), '{#MyAppBuild}', False);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usDone then
    MsgBox('岛仔液态玻璃测试版已卸载。' + #13#10#13#10 +
      '测试数据（含你调过的设置）保留在：' + #13#10 +
      ExpandConstant('{localappdata}\{#MyDataDir}') + #13#10#13#10 +
      '要彻底清理请手动删除该文件夹。', mbInformation, MB_OK);
end;
