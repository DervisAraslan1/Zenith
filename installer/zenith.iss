; Inno Setup script for Zenith. Build it with installer\build.ps1, which
; compiles zenith.exe first and passes the workspace version in.
;
; Installs for the current user by default (no admin prompt), into
; %LOCALAPPDATA%\Programs\Zenith. The first page also offers an install for
; all users.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#define AppName "Zenith"
#define AppExe "zenith.exe"
#define RunKey "Software\Microsoft\Windows\CurrentVersion\Run"

[Setup]
; Never change AppId: it is how Setup finds an earlier install to upgrade.
AppId={{B5E5B4A1-6C90-4520-8C43-D3D66AA936DA}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Zenith
AppCopyright=Copyright (c) 2026 Zenith, based on MIT-licensed code
VersionInfoVersion={#AppVersion}
VersionInfoDescription={#AppName} Setup
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
DisableWelcomePage=no
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile=..\LICENSE
SetupIconFile=..\assets\icon\zenith.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
WizardStyle=modern dynamic
WizardImageFile=art\wizard-100.png,art\wizard-125.png,art\wizard-150.png,art\wizard-200.png
WizardSmallImageFile=art\wizard-small-100.png,art\wizard-small-125.png,art\wizard-small-150.png,art\wizard-small-200.png
; Zenith is stopped in code before files are replaced (see StopZenith).
CloseApplications=no
OutputDir=..\dist
OutputBaseFilename=Zenith-Setup-{#AppVersion}
Compression=lzma2/ultra64
SolidCompression=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
WelcomeLabel2=This will install [name/ver] on your computer.%n%nZenith is a small glass pill at the top of your screen. Hover it and it opens into a grid of utilities.
FinishedLabel=Setup has finished installing [name].%n%nLook for the pill at the top of your screen and hover it to open. CapsLock+N pins it open.

[Tasks]
Name: "startup"; Description: "Launch Zenith when I sign in"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; Same value Settings > Launch at startup writes, so the toggle shows it.
Root: HKCU; Subkey: "{#RunKey}"; ValueType: string; ValueName: "Zenith"; ValueData: """{app}\{#AppExe}"""; Tasks: startup
; zenith:// links for the built-in example plugins. Zenith writes the same
; keys at start; registering here makes the links work before the first launch.
Root: HKCU; Subkey: "Software\Classes\zenith"; ValueType: string; ValueName: ""; ValueData: "URL:Zenith plugin installer"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\zenith"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\zenith\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"",0"
Root: HKCU; Subkey: "Software\Classes\zenith\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Code]
// Asks Zenith to quit, then ends it if it is still running. Zenith saves its
// settings as they change, so a forced stop loses nothing.
procedure StopZenith;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM {#AppExe}', '', SW_HIDE, ewWaitUntilTerminated, Code);
  // 128: no such process.
  if Code <> 128 then
  begin
    Sleep(1500);
    Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#AppExe}', '', SW_HIDE, ewWaitUntilTerminated, Code);
    Sleep(300);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopZenith;
  Result := '';
end;

function InitializeUninstall: Boolean;
begin
  StopZenith;
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if CurUninstallStep = usUninstall then
    // Launch at startup may have been turned on in Settings rather than
    // by Setup, so it is removed whichever way it got there.
    RegDeleteValue(HKCU, '{#RunKey}', 'Zenith');

  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{userappdata}\Zenith');
    if DirExists(DataDir) and not UninstallSilent then
      if MsgBox('Also remove your Zenith settings, notes, clipboard history and plugins?' + #13#10#13#10 +
                'They are in ' + DataDir + '. Keep them if you plan to install Zenith again.',
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir, True, True, True);
  end;
end;
