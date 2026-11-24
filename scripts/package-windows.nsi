; Design Editor Windows installer (NSIS).
;
; Why NSIS (not Inno Setup): the TS reference pipeline
; (apps/desktop/electron-builder.yml) also targets `nsis`, so installer UX
; stays consistent between the Electron and Rust shells. The release workflow
; installs NSIS explicitly before invoking makensis.
;
; Installs:
;   design-editor.exe  editor binary
;   de.exe                  CLI binary (also shipped standalone as
;                           de-cli-<target>.zip in the release; $INSTDIR is
;                           NOT added to PATH — doing so reliably needs the
;                           non-stock EnVar plugin, deferred)
;   design-editor.ico          icon used by shortcuts + the .de/.pen ProgIDs
;   Uninstall.exe           uninstaller (registered in Add/Remove Programs)
;
; Per-user install (#362), like VS Code's user setup, Slack or Discord:
; $LOCALAPPDATA\Programs\Design Editor, current-user shortcuts and registry
; (HKCU), RequestExecutionLevel user. No step of an install or an in-app
; update needs administrator rights, so the app's silent update never waits
; on a UAC prompt nobody sees. The one exception is a missing or outdated
; Visual C++ runtime: its redistributable is per-machine and asks for
; elevation itself.
;
; Earlier releases installed per-machine under Program Files. An interactive
; install finds that copy (HKLM uninstall key) and offers to remove it; its
; uninstaller asks for elevation once. A silent install leaves it alone.
;
; File association: ProgID "DesignEditor.Document" under
; HKCU\Software\Classes with DefaultIcon and an open command, claimed by .de;
; a separate ProgID "DesignEditor.PenDevImport" (same icon + command) is
; claimed by .pen (pen.dev files, which open as an untitled import and are
; never written back). HKCU classes take precedence over machine-level ones
; in the merged HKCR view. `.fig` is deliberately not claimed on Windows
; (macOS-only association).
;
; Updates: the app's helper runs `setup.exe /S /D=<its folder>` after the app
; quits. If the app is still running the installer waits up to 30 s for it
; (asks to close it when interactive) instead of failing on a locked file.
;
; Compile (relative paths resolve against this script's directory, so the
; workflow passes absolute /D defines):
;   makensis "/DVERSION=X.Y.Z" "/DARCH=x64" ^
;     "/DBIN_DIR=D:\w\target\x86_64-pc-windows-msvc\release" ^
;     "/DICON_FILE=D:\w\crates\de-host-desktop\assets\icon.ico" ^
;     "/DVC_REDIST_FILE=D:\w\VC_redist.x64.exe" ^
;     "/DOUT_FILE=D:\w\Design-Editor-X.Y.Z-x64-win-setup.exe" ^
;     scripts\package-windows.nsi
;
; The script is compile-tested with pinned NSIS 3.12 for x64 and ARM64,
; including pre-release VERSION values and the real pinned redistributable.
; Windows CI remains responsible for native install/runtime verification. For
; ARCH=arm64 the installer stub is x86 and runs under emulation on
; Windows-on-ARM; the payload binaries are native aarch64.
;
; VIProductVersion is intentionally omitted: it requires a strict 4-part
; numeric version and would break compiles for pre-release tags like
; X.Y.Z-beta.1.

Unicode true

!include "MUI2.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef ARCH
  !define ARCH "x64"
!endif
!ifndef BIN_DIR
  !define BIN_DIR "..\target\release"
!endif
!ifndef ICON_FILE
  !define ICON_FILE "..\crates\de-host-desktop\assets\icon.ico"
!endif
!ifndef OUT_FILE
  !define OUT_FILE "Design-Editor-${VERSION}-${ARCH}-win-setup.exe"
!endif
!ifndef VC_REDIST_FILE
  !error "VC_REDIST_FILE must point to the staged Microsoft Visual C++ Redistributable"
!endif

; This command intentionally omits /noerrors, so a missing file fails the
; package build. An existing PE without readable version metadata produces
; empty defines, which the explicit checks below also reject. /packed exposes
; the ProductVersion as HIGH and LOW DWORD defines for the runtime code below.
!getdllversion /packed /productversion "${VC_REDIST_FILE}" VC_REDIST_VERSION_
!if "${VC_REDIST_VERSION_HIGH}" == ""
  !error "VC_REDIST_FILE does not contain readable ProductVersion metadata"
!endif
!if "${VC_REDIST_VERSION_LOW}" == ""
  !error "VC_REDIST_FILE does not contain readable ProductVersion metadata"
!endif

!define PRODUCT_NAME "Design Editor"
!define EXE_NAME "design-editor.exe"
!define CLI_NAME "de.exe"
!define PROG_ID "DesignEditor.Document"
!define PEN_PROG_ID "DesignEditor.PenDevImport"
!define REG_APP_KEY "Software\${PRODUCT_NAME}"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"
!define CLASSES_KEY "Software\Classes"
!define VC_RUNTIME_REG_KEY "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\${ARCH}"
!define VC_REDIST_EXE "vc_redist.${ARCH}.exe"
!define VC_REDIST_LOG "$TEMP\Design-Editor-vc-redist-${ARCH}.log"

Var VCRuntimeCompatible
Var VCRuntimeInstalled
Var VCRuntimeInstalledMajor
Var VCRuntimeInstalledMinor
Var VCRuntimeInstalledBuild
Var VCRuntimeInstalledRevision
Var VCRuntimeInstalledVersion
Var VCRuntimeRequiredHigh
Var VCRuntimeRequiredLow
Var VCRuntimeRequiredMajor
Var VCRuntimeRequiredMinor
Var VCRuntimeRequiredBuild
Var VCRuntimeRequiredRevision
Var VCRuntimeRequiredVersion
Var VCRuntimeExitCode

Name "${PRODUCT_NAME}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT_NAME}"
InstallDirRegKey HKCU "${REG_APP_KEY}" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "${ICON_FILE}"
!define MUI_UNICON "${ICON_FILE}"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
; allowToChangeInstallationDirectory parity with electron-builder.yml
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; Set VCRuntimeCompatible to 1 only when the native runtime for the payload
; architecture is installed and its four-part version is at least the bundled
; redistributable's ProductVersion. NSIS uses an x86 installer stub even for
; the ARM64 package, so SetRegView 64 is required to reach the native x64 or
; ARM64 view instead of the stub's redirected 32-bit view.
Function CheckVCRuntime
  StrCpy $VCRuntimeCompatible "0"
  StrCpy $VCRuntimeInstalledVersion "not installed"

  SetRegView 64
  ClearErrors
  ReadRegDWORD $VCRuntimeInstalled HKLM "${VC_RUNTIME_REG_KEY}" "Installed"
  IfErrors vc_runtime_check_done
  IntCmpU $VCRuntimeInstalled 1 vc_runtime_check_version \
    vc_runtime_check_done vc_runtime_check_done

  vc_runtime_check_version:
    ClearErrors
    ReadRegDWORD $VCRuntimeInstalledMajor HKLM "${VC_RUNTIME_REG_KEY}" "Major"
    IfErrors vc_runtime_check_done
    ReadRegDWORD $VCRuntimeInstalledMinor HKLM "${VC_RUNTIME_REG_KEY}" "Minor"
    IfErrors vc_runtime_check_done
    ReadRegDWORD $VCRuntimeInstalledBuild HKLM "${VC_RUNTIME_REG_KEY}" "Bld"
    IfErrors vc_runtime_check_done
    ReadRegDWORD $VCRuntimeInstalledRevision HKLM "${VC_RUNTIME_REG_KEY}" "Rbld"
    IfErrors vc_runtime_check_done

    StrCpy $VCRuntimeInstalledVersion \
      "$VCRuntimeInstalledMajor.$VCRuntimeInstalledMinor.$VCRuntimeInstalledBuild.$VCRuntimeInstalledRevision"

    ; Compare each component with unsigned ordering. This avoids signed DWORD
    ; surprises and does not rely on a lexicographic version-string comparison.
    IntCmpU $VCRuntimeInstalledMajor $VCRuntimeRequiredMajor \
      vc_runtime_check_minor vc_runtime_check_done vc_runtime_check_compatible
  vc_runtime_check_minor:
    IntCmpU $VCRuntimeInstalledMinor $VCRuntimeRequiredMinor \
      vc_runtime_check_build vc_runtime_check_done vc_runtime_check_compatible
  vc_runtime_check_build:
    IntCmpU $VCRuntimeInstalledBuild $VCRuntimeRequiredBuild \
      vc_runtime_check_revision vc_runtime_check_done vc_runtime_check_compatible
  vc_runtime_check_revision:
    IntCmpU $VCRuntimeInstalledRevision $VCRuntimeRequiredRevision \
      vc_runtime_check_compatible vc_runtime_check_done vc_runtime_check_compatible

  vc_runtime_check_compatible:
    StrCpy $VCRuntimeCompatible "1"

  vc_runtime_check_done:
    ; Restore the x86 stub's default view so the installer's existing registry
    ; writes keep their established location.
    SetRegView default
FunctionEnd

;-- Per-user context for shortcuts ($SMPROGRAMS, $DESKTOP).
Function .onInit
  SetShellVarContext current
FunctionEnd

Function un.onInit
  SetShellVarContext current
FunctionEnd

; Offer to remove a per-machine copy left by an earlier release, so the
; Start menu and file associations do not point at two installs. Its
; uninstaller asks for elevation (ExecShellWait "runas"); it copies itself
; to %TEMP% and returns at once, so wait for the app file to go.
Function RemoveMachineInstall
  ClearErrors
  ReadRegStr $0 HKLM "${UNINST_KEY}" "InstallLocation"
  ${If} ${Errors}
  ${OrIf} $0 == ""
    Return
  ${EndIf}
  ${If} $0 == $INSTDIR
    Return
  ${EndIf}
  IfFileExists "$0\Uninstall.exe" 0 machine_done
  IfSilent machine_done
  MessageBox MB_YESNO|MB_ICONQUESTION \
    "An earlier Design Editor is installed for all users in:$\r$\n$0$\r$\n$\r$\nRemove it now? Updates then install without administrator approval. Windows asks for permission once." \
    IDNO machine_done
  ClearErrors
  ExecShellWait "runas" "$0\Uninstall.exe" "/S"
  IfErrors machine_done
  StrCpy $1 0
  machine_wait:
    IfFileExists "$0\${EXE_NAME}" 0 machine_done
    IntOp $1 $1 + 1
    IntCmp $1 120 machine_done
    Sleep 500
    Goto machine_wait
  machine_done:
FunctionEnd

; An update runs while the app may still be closing; a manual install may
; run while it is open. Writing over a running exe fails, so wait for it:
; up to 30 s when silent, else ask the user to close it.
Function WaitForAppClosed
  StrCpy $1 0
  app_check:
    IfFileExists "$INSTDIR\${EXE_NAME}" 0 app_closed
    ClearErrors
    FileOpen $2 "$INSTDIR\${EXE_NAME}" a
    IfErrors app_locked
    FileClose $2
    Goto app_closed
  app_locked:
    IfSilent app_wait
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION \
      "Design Editor is running. Close it, then press Retry." \
      IDRETRY app_check
    Abort "Design Editor is still running."
  app_wait:
    IntOp $1 $1 + 1
    IntCmp $1 60 app_give_up
    Sleep 500
    Goto app_check
  app_give_up:
    Abort "Design Editor is still running."
  app_closed:
FunctionEnd

; Propagate the standard Windows success-with-reboot-required status to silent
; installers and callers that inspect the process exit code. The interactive
; MUI flow also uses the reboot flag to tell the user a restart is required.
Function .onInstSuccess
  IfRebootFlag 0 vc_runtime_no_reboot_exit
  SetErrorLevel 3010
  vc_runtime_no_reboot_exit:
FunctionEnd

Section "Design Editor" SecMain
  SectionIn RO

  ; Embed the bundled redistributable's ProductVersion as two DWORDs, unpack it
  ; into the registry's Major/Minor/Bld/Rbld shape, and stage the prerequisite
  ; in the automatically cleaned NSIS plug-in directory. This all happens
  ; before any Design Editor application file is written to $INSTDIR.
  StrCpy $VCRuntimeRequiredHigh "${VC_REDIST_VERSION_HIGH}"
  StrCpy $VCRuntimeRequiredLow "${VC_REDIST_VERSION_LOW}"
  IntOp $VCRuntimeRequiredMajor $VCRuntimeRequiredHigh >>> 16
  IntOp $VCRuntimeRequiredMinor $VCRuntimeRequiredHigh & 0x0000FFFF
  IntOp $VCRuntimeRequiredBuild $VCRuntimeRequiredLow >>> 16
  IntOp $VCRuntimeRequiredRevision $VCRuntimeRequiredLow & 0x0000FFFF
  StrCpy $VCRuntimeRequiredVersion \
    "$VCRuntimeRequiredMajor.$VCRuntimeRequiredMinor.$VCRuntimeRequiredBuild.$VCRuntimeRequiredRevision"

  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File "/oname=${VC_REDIST_EXE}" "${VC_REDIST_FILE}"

  DetailPrint \
    "Checking Microsoft Visual C++ runtime ${ARCH} (required $VCRuntimeRequiredVersion)"
  Call CheckVCRuntime
  StrCmp $VCRuntimeCompatible "1" vc_runtime_ready

  DetailPrint \
    "Installing Microsoft Visual C++ Redistributable ${ARCH} (found $VCRuntimeInstalledVersion)"
  ClearErrors
  ExecWait \
    '"$PLUGINSDIR\${VC_REDIST_EXE}" /install /passive /norestart /log "${VC_REDIST_LOG}"' \
    $VCRuntimeExitCode
  IfErrors vc_runtime_launch_failed

  ; 0 is success, 3010 is success with a required reboot, and 1638 means a
  ; different product version is already installed. Every nominal success is
  ; verified against the registry; in particular, 1638 is never accepted on
  ; its exit code alone.
  StrCmp $VCRuntimeExitCode "0" vc_runtime_verify_success
  StrCmp $VCRuntimeExitCode "3010" vc_runtime_verify_reboot
  StrCmp $VCRuntimeExitCode "1638" vc_runtime_verify_existing
  Goto vc_runtime_install_failed

  vc_runtime_verify_success:
    Call CheckVCRuntime
    StrCmp $VCRuntimeCompatible "1" vc_runtime_install_succeeded \
      vc_runtime_verification_failed

  vc_runtime_verify_reboot:
    Call CheckVCRuntime
    StrCmp $VCRuntimeCompatible "1" 0 vc_runtime_verification_failed
    SetRebootFlag true
    DetailPrint \
      "Microsoft Visual C++ Redistributable installed; Windows restart required"
    Goto vc_runtime_ready

  vc_runtime_verify_existing:
    Call CheckVCRuntime
    StrCmp $VCRuntimeCompatible "1" vc_runtime_existing_compatible \
      vc_runtime_install_failed

  vc_runtime_install_succeeded:
    DetailPrint \
      "Microsoft Visual C++ runtime $VCRuntimeInstalledVersion installed successfully"
    Goto vc_runtime_ready

  vc_runtime_existing_compatible:
    DetailPrint \
      "Existing Microsoft Visual C++ runtime $VCRuntimeInstalledVersion is compatible"
    Goto vc_runtime_ready

  vc_runtime_launch_failed:
    MessageBox MB_OK|MB_ICONSTOP \
      "The Microsoft Visual C++ Redistributable could not be started.$\r$\n$\r$\nDesign Editor was not installed. See the log for details:$\r$\n${VC_REDIST_LOG}" \
      /SD IDOK
    Abort "Microsoft Visual C++ Redistributable could not be started."

  vc_runtime_verification_failed:
    MessageBox MB_OK|MB_ICONSTOP \
      "The Microsoft Visual C++ Redistributable returned exit code $VCRuntimeExitCode, but runtime $VCRuntimeRequiredVersion was not detected.$\r$\n$\r$\nDesign Editor was not installed. See the log for details:$\r$\n${VC_REDIST_LOG}" \
      /SD IDOK
    Abort "Microsoft Visual C++ Redistributable verification failed."

  vc_runtime_install_failed:
    MessageBox MB_OK|MB_ICONSTOP \
      "The Microsoft Visual C++ Redistributable installation failed with exit code $VCRuntimeExitCode.$\r$\n$\r$\nDesign Editor was not installed. See the log for details:$\r$\n${VC_REDIST_LOG}" \
      /SD IDOK
    Abort "Microsoft Visual C++ Redistributable installation failed."

  vc_runtime_ready:
    DetailPrint \
      "Microsoft Visual C++ runtime $VCRuntimeInstalledVersion satisfies $VCRuntimeRequiredVersion"

  Call RemoveMachineInstall
  Call WaitForAppClosed

  SetOutPath "$INSTDIR"

  File "${BIN_DIR}\${EXE_NAME}"
  File "${BIN_DIR}\${CLI_NAME}"
  File "/oname=design-editor.ico" "${ICON_FILE}"

  ; ANGLE fallback DLLs (libEGL.dll + libGLESv2.dll, optionally
  ; d3dcompiler_47.dll). Installed next to the exe so glutin's EGL path
  ; loads them when the native WGL OpenGL context can't drive Skia — the
  ; machines that were flash-exiting on startup (no/old GPU driver,
  ; software-only OpenGL, VMs, RDP). See
  ; `SharedSkiaContext::new_desktop` for the fallback wiring.
  ;
  ; `/nonfatal`: the release/CI build must stage these DLLs into
  ; ${BIN_DIR} (matching the target arch) before running makensis. Until
  ; that step exists the installer still builds (just without the
  ; fallback), so packaging never hard-breaks on a missing DLL.
  File /nonfatal "${BIN_DIR}\libEGL.dll"
  File /nonfatal "${BIN_DIR}\libGLESv2.dll"
  File /nonfatal "${BIN_DIR}\d3dcompiler_47.dll"

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "${REG_APP_KEY}" "InstallDir" "$INSTDIR"

  ; Shortcuts — createDesktopShortcut / createStartMenuShortcut parity.
  CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
  CreateShortcut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk" \
    "$INSTDIR\${EXE_NAME}" "" "$INSTDIR\design-editor.ico"
  CreateShortcut "$DESKTOP\${PRODUCT_NAME}.lnk" \
    "$INSTDIR\${EXE_NAME}" "" "$INSTDIR\design-editor.ico"

  ; Add/Remove Programs entry.
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\design-editor.ico"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Design Editor contributors"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://github.com/kemalcalak/design-editor"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1

  ; File association: .de documents get their own ProgID.
  WriteRegStr HKCU "${CLASSES_KEY}\${PROG_ID}" "" "Design Editor Document"
  WriteRegStr HKCU "${CLASSES_KEY}\${PROG_ID}\DefaultIcon" "" "$INSTDIR\design-editor.ico"
  WriteRegStr HKCU "${CLASSES_KEY}\${PROG_ID}\shell" "" "open"
  WriteRegStr HKCU "${CLASSES_KEY}\${PROG_ID}\shell\open\command" "" '"$INSTDIR\${EXE_NAME}" "%1"'

  ; pen.dev .pen files are a separate, import-only type with their own ProgID.
  WriteRegStr HKCU "${CLASSES_KEY}\${PEN_PROG_ID}" "" "pen.dev Document (import)"
  WriteRegStr HKCU "${CLASSES_KEY}\${PEN_PROG_ID}\DefaultIcon" "" "$INSTDIR\design-editor.ico"
  WriteRegStr HKCU "${CLASSES_KEY}\${PEN_PROG_ID}\shell" "" "open"
  WriteRegStr HKCU "${CLASSES_KEY}\${PEN_PROG_ID}\shell\open\command" "" '"$INSTDIR\${EXE_NAME}" "%1"'

  WriteRegStr HKCU "${CLASSES_KEY}\.de" "" "${PROG_ID}"
  WriteRegStr HKCU "${CLASSES_KEY}\.de" "Content Type" "application/x-design-editor"
  WriteRegStr HKCU "${CLASSES_KEY}\.pen" "" "${PEN_PROG_ID}"
  WriteRegStr HKCU "${CLASSES_KEY}\.pen" "Content Type" "application/x-pen-dev"

  ; SHCNE_ASSOCCHANGED — tell the shell to refresh icon/association caches.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${EXE_NAME}"
  Delete "$INSTDIR\${CLI_NAME}"
  Delete "$INSTDIR\design-editor.ico"
  Delete "$INSTDIR\libEGL.dll"
  Delete "$INSTDIR\libGLESv2.dll"
  Delete "$INSTDIR\d3dcompiler_47.dll"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk"
  RMDir "$SMPROGRAMS\${PRODUCT_NAME}"
  Delete "$DESKTOP\${PRODUCT_NAME}.lnk"

  ; Only unclaim the extensions if they still point at our ProgID — never
  ; clobber an association another app took over after us.
  ReadRegStr $0 HKCU "${CLASSES_KEY}\.de" ""
  StrCmp $0 "${PROG_ID}" 0 +2
    DeleteRegKey HKCU "${CLASSES_KEY}\.de"
  ReadRegStr $0 HKCU "${CLASSES_KEY}\.pen" ""
  StrCmp $0 "${PEN_PROG_ID}" 0 +2
    DeleteRegKey HKCU "${CLASSES_KEY}\.pen"
  DeleteRegKey HKCU "${CLASSES_KEY}\${PROG_ID}"
  DeleteRegKey HKCU "${CLASSES_KEY}\${PEN_PROG_ID}"

  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "${REG_APP_KEY}"

  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
SectionEnd
