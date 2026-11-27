; 岛仔（自定义构建）安装包 —— Inno Setup 7
; 编译：ISCC.exe daozai.iss
; 特性：默认装到 C:\Program Files\DaozaiCustom（{autopf}，需管理员；2026-11-17 从
;       D:\software\Daozai 硬编码改为软件常用惯例，老用户升级 Inno 会记住原目录）、
;       浏览器扩展桥宿主注册（Chrome/Edge）、开机自启（可勾选）、
;       开始菜单/桌面快捷方式、自带卸载器。
; 用户数据在 %APPDATA%\DaozaiCustom，安装/卸载都不碰。

#define MyAppName "岛仔 DaozaiCustom"
; 版本号由 build-innosetup.ps1 从 Cargo.toml 读出来传进来，避免和 exe 的
; FileVersion 漂移（exe 版本来自 Cargo.toml）。手工编译时才用这里的默认值。
#ifndef MyAppVersion
#define MyAppVersion "1.4.1"
#endif
#define MyAppBuild "custom-isolated"
; 从哪个 cargo profile 的产物目录取 exe（release / dev-release）。
; 命令行覆盖：ISCC.exe /DTargetProfile=dev-release daozai.iss
#ifndef TargetProfile
#define TargetProfile "release"
#endif

[Setup]
AppId={{C7E3A19D-5F42-4B8E-A1D6-9E2C4F7B83A5}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher=自定义构建
DefaultDirName={autopf}\DaozaiCustom
DirExistsWarning=no
; Inno 的 DisableDirPage 默认 auto：检测到旧安装目录就跳过「选择目标位置」页。
; 定制版要让用户每次都能自己指定安装目录，这里显式关掉自动跳过。
DisableDirPage=no
AppendDefaultDirName=no
PrivilegesRequired=admin
DefaultGroupName=岛仔
DisableProgramGroupPage=yes
OutputDir=.
OutputBaseFilename=daozai-setup-{#MyAppBuild}
SetupIconFile=..\assets\daozai.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
UninstallDisplayName={#MyAppName} {#MyAppVersion}
UninstallDisplayIcon={app}\daozai.ico

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "autostart"; Description: "开机自动启动岛仔"; GroupDescription: "其他："; Flags: checkedonce
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "其他："; Flags: unchecked

[Files]
Source: "..\target\{#TargetProfile}\daozai.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\daozai.ico"; DestDir: "{app}"; Flags: ignoreversion
; 浏览器扩展桥宿主：独立精简版二进制（不链接 GUI 栈，约 460KB），复制为 daozai_bridge.exe。
Source: "..\target\{#TargetProfile}\daozai-bridge.exe"; DestDir: "{app}\bridge"; DestName: "daozai_bridge.exe"; Flags: ignoreversion
; 无线设备电量组件：Rust 实现（约 514KB），随应用分发，主程序直接启动。
Source: "..\vendor\halo-rust\target\{#TargetProfile}\daozai-halo.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\browser-bridge\extension\manifest.json"; DestDir: "{app}\bridge\extension"; Flags: ignoreversion
Source: "..\browser-bridge\extension\sw.js"; DestDir: "{app}\bridge\extension"; Flags: ignoreversion
Source: "INSTALL-NOTES.txt"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Registry]
; 浏览器扩展桥宿主：Chrome 与 Edge 各注册一份（HKCU，无需管理员）
Root: HKCU; Subkey: "Software\Google\Chrome\NativeMessagingHosts\com.daozai.island"; ValueType: string; ValueData: "{app}\bridge\com.daozai.island.json"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Microsoft\Edge\NativeMessagingHosts\com.daozai.island"; ValueType: string; ValueData: "{app}\bridge\com.daozai.island.json"; Flags: uninsdeletekey
; 开机自启（与之前手动建的 Run 项同名，安装即接管）
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Daozai"; ValueData: """{app}\daozai.exe"""; Tasks: autostart; Flags: uninsdeletevalue

[Icons]
Name: "{group}\岛仔"; Filename: "{app}\daozai.exe"; IconFilename: "{app}\daozai.ico"
Name: "{group}\浏览器扩展（加载这个文件夹）"; Filename: "{app}\bridge\extension"
Name: "{autodesktop}\岛仔"; Filename: "{app}\daozai.exe"; IconFilename: "{app}\daozai.ico"; Tasks: desktopicon

; [Code] 在安装时生成的文件不在 [Files] 清单里，卸载时要显式删掉
[UninstallDelete]
Type: files; Name: "{app}\bridge\daozai_bridge.exe"
Type: files; Name: "{app}\daozai-halo.exe"
Type: files; Name: "{app}\bridge\com.daozai.island.json"
Type: files; Name: "{app}\CUSTOM_BUILD.txt"
Type: dirifempty; Name: "{app}\bridge"
Type: dirifempty; Name: "{app}\bridge\extension"

[Run]
Filename: "{app}\daozai.exe"; Description: "启动岛仔"; Flags: nowait postinstall skipifsilent

[Code]
// 宿主清单要在装完后用真实路径生成（com.daozai.island.json 里 JSON 字符串的路径必须双反斜杠）。
// daozai_bridge.exe 已由 [Files] 直接打包（独立精简版），不再从 daozai.exe 复制。
// 清单内容保持纯 ASCII（JSON 里不能有转义坑，也避开 ANSI/UTF8 编码问题）。
procedure CurStepChanged(CurStep: TSetupStep);
var
  manifest, exePath, jsonPath: string;
  resultCode: Integer;
begin
  if CurStep = ssPostInstall then
  begin
    exePath := ExpandConstant('{app}\bridge\daozai_bridge.exe');
    jsonPath := ExpandConstant('{app}\bridge\com.daozai.island.json');
    if not ForceDirectories(ExpandConstant('{app}\bridge')) then
    begin
      MsgBox('无法创建 bridge 目录。', mbError, MB_OK);
      exit;
    end;
    // 浏览器扩展在线时桥进程一直运行，锁住目标 exe 会让文件覆盖静默失败
    // （静默安装下弹窗也看不见），升级就变成「装了新版桥还是旧的」。先结束它；
    // 扩展下次收发消息时浏览器会自动重新拉起，无感恢复。
    Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM daozai_bridge.exe /F',
      '', SW_HIDE, ewWaitUntilTerminated, resultCode);

    StringChangeEx(exePath, '\', '\\', True);
    manifest := '{"name":"com.daozai.island",' +
      '"description":"Daozai status bridge for browser extensions",' +
      '"path":"' + exePath + '",' +
      '"type":"stdio",' +
      '"allowed_origins":["chrome-extension://dcmheeclfeihicoaakoffjbbepfdfcin/"]}';
    SaveStringToFile(jsonPath, manifest, False);
    SaveStringToFile(ExpandConstant('{app}\CUSTOM_BUILD.txt'), '{#MyAppBuild}', False);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usDone then
    MsgBox('岛仔已卸载。用户数据（设置、通知记录等）保留在 %APPDATA%\DaozaiCustom，可手动删除。', mbInformation, MB_OK);
end;
