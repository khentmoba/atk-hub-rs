; ATK HUB Rust port per-user installer for Windows.
;
; Built by installer\build-nsis.ps1 with:
;   makensis /DVERSION=<x.y.z> /DSTAGING=<staging dir> /DOUTFILE=<setup.exe> installer\AtkHub.nsi
;
; Installs to %LOCALAPPDATA%\Programs\ATK HUB Rust, so no administrator
; rights or UAC prompt are needed. Supports /S for silent installs.

Unicode true
SetCompressor /SOLID lzma

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef STAGING
  !error "Pass /DSTAGING=<staging directory> to makensis"
!endif
!ifndef OUTFILE
  !define OUTFILE "ATK-HUB-setup.exe"
!endif

!define APP_NAME "ATK HUB Rust"
!define APP_EXE "atk-hub-rs.exe"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\AtkHubRust"

Name "${APP_NAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
RequestExecutionLevel user

!include "MUI2.nsh"

!define MUI_ICON "${__FILEDIR__}\..\..\assets\atk-hub.ico"
!define MUI_UNICON "${__FILEDIR__}\..\..\assets\atk-hub.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Start ATK HUB"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  IfFileExists "$INSTDIR\${APP_EXE}" +1 +7
    ClearErrors
    FileOpen $0 "$INSTDIR\${APP_EXE}" a
    IfErrors 0 +3
    MessageBox MB_OK|MB_ICONSTOP "ATK HUB is running. Close it, then run this installer again." /SD IDOK
    Abort
    FileClose $0

  IfFileExists "$INSTDIR\uninstall.exe" +2 0
  IfFileExists "$INSTDIR\${APP_EXE}" 0 +2
  RMDir /r "$INSTDIR"

  SetOutPath "$INSTDIR"
  File /r /x "setup.exe" "${STAGING}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "khentmoba"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "un.Uninstall"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  RMDir /r "$INSTDIR"
SectionEnd
