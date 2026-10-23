#define MyAppName "TikTok Lead Finder"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "LeadFinder Bulgaria"
#define MyAppURL "https://github.com/MoRDekai1364/TikTok_Scan"
#define MyAppExeName "leadfinder-desktop.exe"
#define MyCoreExeName "leadfinder.exe"

[Setup]
AppId={{8F4E2C91-5B74-4D3A-9E12-7C6B1D04A85F}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\TikTok_Lead_Finder
UninstallDisplayIcon={app}\{#MyAppExeName}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes
LicenseFile=installer\LICENSE.txt
InfoBeforeFile=installer\Hello.txt
InfoAfterFile=installer\Finish.txt
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=Setup_Output
OutputBaseFilename=Setup_TikTok_Lead_Finder_Official_Release
SetupIconFile=ui\src-tauri\icons\icon.ico
SolidCompression=yes
WizardStyle=modern stellar

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "bootstrapdeps"; Description: "Install and verify required local media, OCR, speech, and Bulgarian lexicon tools (yt-dlp, ffmpeg, whisper.cpp, llama.cpp, Tesseract BG+EN, BG morphology & IPA dictionary)"; GroupDescription: "Dependencies:"
Name: "downloadmodels"; Description: "Download recommended Whisper ASR + BgGPT LLM models, compile full Bulgarian Wiktionary lexicon, and calibrate hardware during setup"; GroupDescription: "Dependencies:"; Flags: unchecked

[Files]
Source: "release_stage\*"; DestDir: "{app}"; Excludes: "*.pdb,logs\*,data\*,cache\*"; Flags: ignoreversion recursesubdirs createallsubdirs

[Dirs]
Name: "{localappdata}\TikTok_Lead_Finder"
Name: "{localappdata}\TikTok_Lead_Finder\data"
Name: "{localappdata}\TikTok_Lead_Finder\logs"
Name: "{localappdata}\TikTok_Lead_Finder\cache"
Name: "{localappdata}\TikTok_Lead_Finder\tools"
Name: "{localappdata}\TikTok_Lead_Finder\models"

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyCoreExeName}"; Parameters: "setup-deps --non-interactive --ensure-webview2"; StatusMsg: "Verifying and installing required local tools..."; Flags: runhidden waituntilterminated; Tasks: bootstrapdeps
Filename: "{app}\{#MyCoreExeName}"; Parameters: "setup-deps --non-interactive --with-models --calibrate"; StatusMsg: "Downloading speech/LLM models and calibrating hardware..."; Flags: runhidden waituntilterminated; Tasks: downloadmodels
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
