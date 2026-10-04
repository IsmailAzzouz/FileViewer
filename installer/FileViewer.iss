; =====================================================================
; FileViewer Inno Setup Configuration Script
; Supports:
; - Context menu integration ("Open with FileViewer")
; - File associations for JSON, YAML, and TOML
; - Start Menu and Desktop shortcuts
; - PATH environment variable addition
; - Clean uninstallation without residue
; - High-DPI modern wizard styling
; =====================================================================

#define MyAppName "FileViewer"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "Ismail Azzouz"
#define MyAppURL "https://github.com/ismailazzouz/FileViewer"
#define MyAppExeName "file-viewer.exe"

[Setup]
; App Identity
AppId={{8B266491-0309-4C4B-B61E-74B64B11E9B1}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}

; Destination Directories & Permissions
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=FileViewer-Setup-{#MyAppVersion}
SetupIconFile=..\assets\installer\setup-icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}

; Visuals & Modern Styling
WizardStyle=modern
WizardImageFile=..\assets\installer\wizard-large.bmp
WizardSmallImageFile=..\assets\installer\wizard-small.bmp
DisableWelcomePage=no

; Architecture: 64-bit Windows
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

; Compression
Compression=lzma2/ultra64
SolidCompression=yes

; User Privileges: Dual-mode installer (All users or current user)
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline

; Windows Shell Notifications
ChangesAssociations=yes
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
; Shortcuts
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

; Explorer Context Menu
Name: "contextmenu"; Description: "Add ""Open with FileViewer"" to Windows Explorer context menu"; GroupDescription: "Windows Explorer Integration:"

; Default File Associations
Name: "assoc_json"; Description: "JSON files (.json, .jsonc, .jsonl, .ndjson)"; GroupDescription: "File Associations (open by default with FileViewer):"
Name: "assoc_yaml"; Description: "YAML files (.yaml, .yml)"; GroupDescription: "File Associations (open by default with FileViewer):"
Name: "assoc_toml"; Description: "TOML files (.toml)"; GroupDescription: "File Associations (open by default with FileViewer):"

; PATH variable
Name: "addtopath"; Description: "Add FileViewer installation folder to PATH"; GroupDescription: "System Integration:"; Flags: unchecked

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icon.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\samples\*"; DestDir: "{app}\samples"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; -------------------------------------------------------------
; Context Menu: "Open with FileViewer"
; -------------------------------------------------------------
Root: HKA; Subkey: "Software\Classes\*\shell\OpenWithFileViewer"; ValueType: string; ValueName: ""; ValueData: "Open with FileViewer"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\OpenWithFileViewer"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\OpenWithFileViewer\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: contextmenu

; -------------------------------------------------------------
; ProgID Registrations
; -------------------------------------------------------------
; JSON ProgID
Root: HKA; Subkey: "Software\Classes\FileViewer.json"; ValueType: string; ValueName: ""; ValueData: "JSON File"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.json\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.json\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.json\Application"; ValueType: string; ValueName: "ApplicationCompany"; ValueData: "{#MyAppPublisher}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.json\Application"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey

; YAML ProgID
Root: HKA; Subkey: "Software\Classes\FileViewer.yaml"; ValueType: string; ValueName: ""; ValueData: "YAML File"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.yaml\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.yaml\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.yaml\Application"; ValueType: string; ValueName: "ApplicationCompany"; ValueData: "{#MyAppPublisher}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.yaml\Application"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey

; TOML ProgID
Root: HKA; Subkey: "Software\Classes\FileViewer.toml"; ValueType: string; ValueName: ""; ValueData: "TOML File"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.toml\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.toml\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.toml\Application"; ValueType: string; ValueName: "ApplicationCompany"; ValueData: "{#MyAppPublisher}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\FileViewer.toml\Application"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey

; -------------------------------------------------------------
; Registered Applications & Supported Types
; -------------------------------------------------------------
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".json"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".jsonc"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".jsonl"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".ndjson"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".yaml"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".yml"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".toml"; ValueData: ""; Flags: uninsdeletevalue

; OpenWithProgids registrations (makes FileViewer appear in Open With choices)
Root: HKA; Subkey: "Software\Classes\.json\OpenWithProgids"; ValueType: string; ValueName: "FileViewer.json"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.jsonc\OpenWithProgids"; ValueType: string; ValueName: "FileViewer.json"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.jsonl\OpenWithProgids"; ValueType: string; ValueName: "FileViewer.json"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.ndjson\OpenWithProgids"; ValueType: string; ValueName: "FileViewer.json"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.yaml\OpenWithProgids"; ValueType: string; ValueName: "FileViewer.yaml"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.yml\OpenWithProgids"; ValueType: string; ValueName: "FileViewer.yaml"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.toml\OpenWithProgids"; ValueType: string; ValueName: "FileViewer.toml"; ValueData: ""; Flags: uninsdeletevalue

; -------------------------------------------------------------
; Default Associations (Active when tasks selected)
; -------------------------------------------------------------
Root: HKA; Subkey: "Software\Classes\.json"; ValueType: string; ValueName: ""; ValueData: "FileViewer.json"; Flags: uninsdeletevalue; Tasks: assoc_json
Root: HKA; Subkey: "Software\Classes\.jsonc"; ValueType: string; ValueName: ""; ValueData: "FileViewer.json"; Flags: uninsdeletevalue; Tasks: assoc_json
Root: HKA; Subkey: "Software\Classes\.jsonl"; ValueType: string; ValueName: ""; ValueData: "FileViewer.json"; Flags: uninsdeletevalue; Tasks: assoc_json
Root: HKA; Subkey: "Software\Classes\.ndjson"; ValueType: string; ValueName: ""; ValueData: "FileViewer.json"; Flags: uninsdeletevalue; Tasks: assoc_json

Root: HKA; Subkey: "Software\Classes\.yaml"; ValueType: string; ValueName: ""; ValueData: "FileViewer.yaml"; Flags: uninsdeletevalue; Tasks: assoc_yaml
Root: HKA; Subkey: "Software\Classes\.yml"; ValueType: string; ValueName: ""; ValueData: "FileViewer.yaml"; Flags: uninsdeletevalue; Tasks: assoc_yaml

Root: HKA; Subkey: "Software\Classes\.toml"; ValueType: string; ValueName: ""; ValueData: "FileViewer.toml"; Flags: uninsdeletevalue; Tasks: assoc_toml

; -------------------------------------------------------------
; Windows Capabilities & Default Programs Registration
; -------------------------------------------------------------
Root: HKA; Subkey: "Software\FileViewer\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Fast GPU-accelerated viewer & editor for JSON, YAML, and TOML"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\FileViewer\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\FileViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".json"; ValueData: "FileViewer.json"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\FileViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".jsonc"; ValueData: "FileViewer.json"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\FileViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".jsonl"; ValueData: "FileViewer.json"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\FileViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".ndjson"; ValueData: "FileViewer.json"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\FileViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".yaml"; ValueData: "FileViewer.yaml"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\FileViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".yml"; ValueData: "FileViewer.yaml"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\FileViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".toml"; ValueData: "FileViewer.toml"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "FileViewer"; ValueData: "Software\FileViewer\Capabilities"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[Code]
// Helper functions for safe PATH management
const
  EnvironmentKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

function IsInPath(PathList, Dir: String): Boolean;
var
  P: Integer;
begin
  PathList := ';' + Uppercase(PathList) + ';';
  Dir := ';' + Uppercase(Dir) + ';';
  P := Pos(Dir, PathList);
  Result := (P > 0);
end;

procedure AddAppToPath();
var
  Paths: String;
  AppDir: String;
  RootKeyVal: Integer;
  SubKeyVal: String;
begin
  AppDir := ExpandConstant('{app}');

  if IsAdminInstallMode() then
  begin
    RootKeyVal := HKEY_LOCAL_MACHINE;
    SubKeyVal := EnvironmentKey;
  end
  else
  begin
    RootKeyVal := HKEY_CURRENT_USER;
    SubKeyVal := 'Environment';
  end;

  if RegQueryStringValue(RootKeyVal, SubKeyVal, 'Path', Paths) then
  begin
    if not IsInPath(Paths, AppDir) then
    begin
      if (Length(Paths) > 0) and (Paths[Length(Paths)] <> ';') then
        Paths := Paths + ';';
      Paths := Paths + AppDir;
      RegWriteExpandStringValue(RootKeyVal, SubKeyVal, 'Path', Paths);
    end;
  end
  else
  begin
    RegWriteExpandStringValue(RootKeyVal, SubKeyVal, 'Path', AppDir);
  end;
end;

procedure RemoveAppFromPath();
var
  Paths, NewPaths, AppDir, Target: String;
  P: Integer;
  RootKeyVal: Integer;
  SubKeyVal: String;
begin
  AppDir := ExpandConstant('{app}');

  if IsAdminInstallMode() then
  begin
    RootKeyVal := HKEY_LOCAL_MACHINE;
    SubKeyVal := EnvironmentKey;
  end
  else
  begin
    RootKeyVal := HKEY_CURRENT_USER;
    SubKeyVal := 'Environment';
  end;

  if RegQueryStringValue(RootKeyVal, SubKeyVal, 'Path', Paths) then
  begin
    NewPaths := Paths;
    Target := ';' + AppDir + ';';
    Paths := ';' + Paths + ';';
    P := Pos(Uppercase(Target), Uppercase(Paths));
    if P > 0 then
    begin
      Delete(Paths, P, Length(Target) - 1);
      NewPaths := Copy(Paths, 2, Length(Paths) - 2);
      RegWriteExpandStringValue(RootKeyVal, SubKeyVal, 'Path', NewPaths);
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('addtopath') then
    begin
      AddAppToPath();
    end;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveAppFromPath();
  end;
end;
