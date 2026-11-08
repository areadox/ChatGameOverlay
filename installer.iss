; Inno-Setup-Skript für ChatGameOverlay
; Wird von GitHub Actions gebaut:  iscc /DMyAppVersion=0.0.3 installer.iss
;
; Installation pro Benutzer (keine Admin-Rechte nötig) nach
; %LOCALAPPDATA%\Programs\ChatGameOverlay – dort darf sich das Programm
; auch selbst aktualisieren.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#define MyAppName "ChatGameOverlay"
#define MyAppExe "ChatGameOverlay.exe"

[Setup]
AppId={{8F3C2A71-5B4E-4C9D-9E21-7A6B0D3F4C12}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher=Areadox
AppPublisherURL=https://github.com/areadox/ChatGameOverlay
AppSupportURL=https://github.com/areadox/ChatGameOverlay/issues
AppUpdatesURL=https://github.com/areadox/ChatGameOverlay/releases
DefaultDirName={localappdata}\Programs\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=dist
OutputBaseFilename=ChatGameOverlay-Setup
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
UninstallDisplayName={#MyAppName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Sprache des Installers richtet sich nach der Windows-Sprache
ShowLanguageDialog=auto
CloseApplications=yes
RestartApplications=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
german.DesktopIcon=Verknüpfung auf dem Desktop anlegen
english.DesktopIcon=Create a desktop shortcut
german.ExtraIcons=Zusätzliche Verknüpfungen:
english.ExtraIcons=Additional shortcuts:
german.Autostart=Mit Windows starten
english.Autostart=Start with Windows
german.StartGroup=Start:
english.StartGroup=Startup:
german.Uninstall=deinstallieren
english.Uninstall=uninstall
german.LaunchNow=jetzt starten
english.LaunchNow=launch now

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; GroupDescription: "{cm:ExtraIcons}"
Name: "autostart"; Description: "{cm:Autostart}"; GroupDescription: "{cm:StartGroup}"; Flags: unchecked

[Files]
Source: "dist\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\{#MyAppName} {cm:Uninstall}"; Filename: "{uninstallexe}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: """{app}\{#MyAppExe}"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "{#MyAppName} {cm:LaunchNow}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Reste eines abgebrochenen Updates
Type: files; Name: "{app}\.ChatGameOverlay-update.tmp"
