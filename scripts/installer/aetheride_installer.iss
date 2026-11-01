; Inno Setup 6 Script for AetherIDE Beta.2
; Architecture: Windows 10/11 x64 (MSVC)
; Standard: Non-administrative per-user installation with dependency detection, shell branding, and zero-telemetry preservation.

#define MyAppName "AetherIDE"
#define MyAppVersion "0.1.0-beta.2"
#define MyAppPublisher "AetherIDE Open Source Project"
#define MyAppURL "https://github.com/Imagen2dragon/Aether-IDE"
#define MyAppExeName "AetherIDE.exe"
#define MyAppCliName "aether-cli.exe"

[Setup]
AppId={{D37F20B1-6548-4C8D-9E9A-0A7B5F8C1D02}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\Programs\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=no
AllowNoIcons=yes
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist
OutputBaseFilename=AetherIDE-Setup-{#MyAppVersion}-x64
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ChangesEnvironment=yes
CloseApplications=yes
RestartApplications=no
SetupIconFile=..\..\branding\windows\aetheride-installer.ico
WizardImageFile=..\..\branding\windows\aetheride-wizard.bmp
WizardSmallImageFile=..\..\branding\windows\aetheride-wizard-small.bmp
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName} (v{#MyAppVersion})

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "addtopath"; Description: "Add Aether CLI ({app}\bin) to User PATH environment variable"; GroupDescription: "Command Line Integration:"; Flags: unchecked
Name: "contextmenu"; Description: "Add 'Open with AetherIDE' to Windows Explorer context menu"; GroupDescription: "Windows Shell Integration:"; Flags: unchecked

[Files]
Source: "..\..\dist\windows-x64\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; AppUserModelId: "AetherIDE.Desktop"
Name: "{autoprograms}\{#MyAppName}\AetherIDE Documentation"; Filename: "{#MyAppURL}"; IconFilename: "{app}\{#MyAppExeName}"
Name: "{autoprograms}\{#MyAppName}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; AppUserModelId: "AetherIDE.Desktop"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[Registry]
; User PATH environment variable configuration
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}\bin"; Tasks: addtopath; Check: NeedsAddPath(ExpandConstant('{app}\bin'))

; Context menu for Directory background (clicking empty space inside a folder)
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\AetherIDE"; ValueType: string; ValueName: ""; ValueData: "Open with AetherIDE"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\AetherIDE"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\AetherIDE\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%V"""; Flags: uninsdeletekey; Tasks: contextmenu

; Context menu for Directory (right-clicking a folder)
Root: HKCU; Subkey: "Software\Classes\Directory\shell\AetherIDE"; ValueType: string; ValueName: ""; ValueData: "Open with AetherIDE"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\AetherIDE"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\AetherIDE\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: contextmenu

[Code]
// Helper function to check whether a directory is already in the User PATH
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath)
  then begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + UpperCase(Param) + ';', ';' + UpperCase(OrigPath) + ';') = 0;
end;

// Function to detect whether Microsoft Edge WebView2 Evergreen Runtime is installed
// Strictly queries Microsoft's official WebView2 Evergreen GUID: {F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}
function IsWebView2Installed: Boolean;
var
  Version: string;
begin
  Result := False;

  // 1. Check 64-bit and 32-bit registry keys for official WebView2 Evergreen runtime GUID
  if RegQueryStringValue(HKEY_LOCAL_MACHINE, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) then
  begin
    if (Length(Version) > 0) and (Version <> '0.0.0.0') then
    begin
      Result := True;
      exit;
    end;
  end;

  if RegQueryStringValue(HKEY_LOCAL_MACHINE, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) then
  begin
    if (Length(Version) > 0) and (Version <> '0.0.0.0') then
    begin
      Result := True;
      exit;
    end;
  end;

  if RegQueryStringValue(HKEY_CURRENT_USER, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) then
  begin
    if (Length(Version) > 0) and (Version <> '0.0.0.0') then
    begin
      Result := True;
      exit;
    end;
  end;

  // 2. Direct binary existence check for msedgewebview2.exe in official EdgeWebView directories
  // Note: We deliberately do NOT accept msedge.exe (Edge browser) as proof of WebView2 Runtime
  if FileExists(ExpandConstant('{pf}\Microsoft\EdgeWebView\Application\msedgewebview2.exe')) or
     FileExists(ExpandConstant('{pf32}\Microsoft\EdgeWebView\Application\msedgewebview2.exe')) then
  begin
    Result := True;
    exit;
  end;
end;

// Setup initialization hook for dependency checking
function InitializeSetup(): Boolean;
var
  ErrorCode: Integer;
begin
  Result := True;

  if not IsWebView2Installed then
  begin
    if MsgBox('Microsoft Edge WebView2 Evergreen Runtime was not detected on this computer.' + #13#10#13#10 +
              'AetherIDE requires WebView2 for its native workbench interface.' + #13#10#13#10 +
              'Would you like to open the official Microsoft WebView2 download page now?',
              mbConfirmation, MB_YESNO) = idYes then
    begin
      ShellExec('open', 'https://go.microsoft.com/fwlink/p/?LinkId=2124703', '', '', SW_SHOWNORMAL, ewNoWait, ErrorCode);
    end;
  end;
end;

// Cleanup User PATH on uninstallation
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  OrigPath, AppBinPath: string;
  PosIndex, LenBin: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    AppBinPath := ExpandConstant('{app}\bin');
    if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
    begin
      LenBin := Length(AppBinPath);
      PosIndex := Pos(AppBinPath, OrigPath);
      if PosIndex > 0 then
      begin
        if (PosIndex > 1) and (OrigPath[PosIndex - 1] = ';') then
        begin
          Delete(OrigPath, PosIndex - 1, LenBin + 1);
        end
        else if (Length(OrigPath) >= PosIndex + LenBin) and (OrigPath[PosIndex + LenBin] = ';') then
        begin
          Delete(OrigPath, PosIndex, LenBin + 1);
        end
        else
        begin
          Delete(OrigPath, PosIndex, LenBin);
        end;
        RegWriteStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath);
      end;
    end;
  end;
end;
