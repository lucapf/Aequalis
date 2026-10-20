; The Windows installer (Inno Setup 6): aequalis-tray.exe, the input box in the notification area,
; and aequalis.exe, the command line; for the current user, with no administrator needed.
;
;   cargo build --release -p aequalis -p aequalis-tray
;   ISCC /DVersion=0.2.9 installer\windows\aequalis.iss
;
; makes dist\aequalis-0.2.9-x86_64-windows-setup.exe. The settings and the downloaded data
; (%APPDATA%\aequalis) stay when it is uninstalled.

#ifndef Version
  #define Version "0.1.0"
#endif
; The numbers of the file's version information (four numbers; a build between releases, `0.2.11-dev.57`,
; has `0.2.11.57`); by default the version itself.
#ifndef VersionInfo
  #define VersionInfo Version
#endif
#define Root "..\.."

[Setup]
; The installer's identity: never change it (Windows would see another program).
AppId={{6B0E5F3A-4C1D-4E9B-9C55-2A7E3F8D1B42}
AppName=Aequalis
AppVersion={#Version}
AppVerName=Aequalis {#Version}
AppPublisher=Luca Palanca Falsini
AppPublisherURL=https://github.com/lucapf/aequalis
AppSupportURL=https://github.com/lucapf/aequalis/issues
AppUpdatesURL=https://github.com/lucapf/aequalis/releases
VersionInfoVersion={#VersionInfo}
VersionInfoProductName=Aequalis
; For the current user: %LOCALAPPDATA%\Programs\Aequalis.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Aequalis
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Windows 10, version 1903: the first with the ICU library that matches the time zones' names.
MinVersion=10.0.18362
OutputDir={#Root}\dist
OutputBaseFilename=aequalis-{#Version}-x86_64-windows-setup
SetupIconFile={#Root}\iconset\icons\windows\aequalis.ico
UninstallDisplayIcon={app}\aequalis-tray.exe
UninstallDisplayName=Aequalis
LicenseFile={#Root}\LICENSE
WizardStyle=modern
Compression=lzma2/ultra64
SolidCompression=yes
; The PATH changes: programs started afterwards see it.
ChangesEnvironment=yes
; An update closes the running app (it does not start it again: the last page offers to).
CloseApplications=force
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "autostart"; Description: "Start Aequalis when I sign in (its icon in the notification area; Win+Alt+A opens it)"
Name: "path"; Description: "Add the aequalis command to the PATH (for the terminal)"

[Files]
Source: "{#Root}\target\release\aequalis-tray.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\target\release\aequalis.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"
Source: "{#Root}\docs\Accepted_Syntax.md"; DestDir: "{app}"

[Icons]
Name: "{autoprograms}\Aequalis"; Filename: "{app}\aequalis-tray.exe"; Comment: "Units, calculator, dates and time zones, answered as you type"

[Registry]
; The same value as the app's own "Start with Windows" (apps/windows/src/system.rs).
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Aequalis"; ValueData: """{app}\aequalis-tray.exe"" --autostart"; Flags: uninsdeletevalue; Tasks: autostart
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}')); Tasks: path

[Run]
Filename: "{app}\aequalis-tray.exe"; Description: "Open Aequalis now"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; The app is stopped before its files go.
Filename: "{sys}\taskkill.exe"; Parameters: "/f /im aequalis-tray.exe"; Flags: runhidden; RunOnceId: "StopTray"

[Code]
{ Whether the user's PATH lacks Dir. }
function NeedsAddPath(Dir: string): Boolean;
var
  Path: string;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Path) then
    Result := True
  else
    Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Path) + ';') = 0;
end;

{ Dir out of the user's PATH (the uninstaller). }
procedure RemoveFromPath(Dir: string);
var
  Path: string;
  At: Integer;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Path) then
    exit;
  Path := ';' + Path + ';';
  At := Pos(';' + Uppercase(Dir) + ';', Uppercase(Path));
  if At = 0 then
    exit;
  Delete(Path, At, Length(Dir) + 1);
  RegWriteExpandStringValue(HKCU, 'Environment', 'Path', Copy(Path, 2, Length(Path) - 2));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
