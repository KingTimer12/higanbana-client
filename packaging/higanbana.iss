; Instalador do Higanbana (Inno Setup 6).
; Compilar: ISCC.exe /DAppVersion=0.1.0 packaging\higanbana.iss   (o CI lê a versão do arquivo VERSION)
; Espera `dist\windows\higanbana.exe` e `dist\windows\lib\libmpv-2.dll`.

#ifndef AppVersion
  #error Passe /DAppVersion=x.y.z (versão do arquivo VERSION).
#endif

[Setup]
; Não mude o AppId: é ele que faz a próxima versão atualizar a instalada.
AppId=Dev.KingTimer12.Higanbana
AppName=Higanbana
AppVersion={#AppVersion}
AppPublisher=KingTimer12
AppPublisherURL=https://github.com/KingTimer12/higanbana-client
; Por usuário (sem pedir administrador): a atualização automática roda sem elevação.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Higanbana
DefaultGroupName=Higanbana
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=Higanbana-Setup-{#AppVersion}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
; Fecha o app aberto antes de trocar os arquivos (usado pela atualização automática).
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\higanbana.exe

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Files]
Source: "..\dist\windows\higanbana.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\windows\lib\*"; DestDir: "{app}\lib"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\Higanbana"; Filename: "{app}\higanbana.exe"

[Run]
; Instalação manual: oferece abrir ao final.
Filename: "{app}\higanbana.exe"; Description: "Abrir o Higanbana"; Flags: nowait postinstall skipifsilent
; Atualização automática (silenciosa): abre o app novo sozinho.
Filename: "{app}\higanbana.exe"; Flags: nowait runasoriginaluser; Check: WizardSilent
