!include "x64.nsh"
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"

!define UNICODE
!define PRODUCT_NAME "Neuron Encrypt"
!define PRODUCT_EXE "neuron-encrypt.exe"
!define PRODUCT_PUBLISHER "Neuron Encrypt Contributors"
!define VERSION "1.0.0"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\NeuronEncrypt"
!define PRODUCT_UNINST_ROOT_KEY "HKLM"
!define VX2_CLASS "NeuronEncrypt.vx2file"

Name "${PRODUCT_NAME} ${VERSION}"
OutFile "NeuronEncrypt-Windows-x64-Setup.exe"
InstallDir "$PROGRAMFILES64\NeuronEncrypt"
InstallDirRegKey HKLM "Software\NeuronEncrypt" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

Var ReinstallDetected
Var InstallMessageLabel

!define MUI_ABORTWARNING
!define MUI_ICON "..\neuron-encrypt\assets\icon.ico"
!define MUI_UNICON "..\neuron-encrypt\assets\icon.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${PRODUCT_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Launch ${PRODUCT_NAME}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
Page custom ShowInstallState
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "This application requires a 64-bit version of Windows."
    Abort
  ${EndIf}
  ReadRegStr $0 HKLM "Software\NeuronEncrypt" "InstallDir"
  ${If} $0 != ""
    StrCpy $INSTDIR $0
    StrCpy $ReinstallDetected "1"
  ${Else}
    StrCpy $ReinstallDetected "0"
  ${EndIf}
FunctionEnd

Function ShowInstallState
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}
  ${If} $ReinstallDetected == "1"
    ${NSD_CreateLabel} 0 0 100% 16u "Updating existing installation…"
  ${Else}
    ${NSD_CreateLabel} 0 0 100% 16u "Installing Neuron Encrypt…"
  ${EndIf}
  Pop $InstallMessageLabel
  nsDialogs::Show
FunctionEnd

Section "Core Application (required)" SEC_CORE
  SectionIn RO
  SetOutPath "$INSTDIR"
  ${If} $ReinstallDetected == "1"
    nsExec::Exec 'taskkill /F /IM neuron-encrypt.exe /T'
  ${EndIf}
  File "..\neuron-encrypt\target\x86_64-pc-windows-msvc\release\neuron-encrypt.exe"
  File /oname=LICENSE.txt "..\LICENSE"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_EXE}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall.lnk" "$INSTDIR\uninstall.exe"

  WriteRegStr HKLM "Software\NeuronEncrypt" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\NeuronEncrypt" "Version" "${VERSION}"

  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\${PRODUCT_EXE},0"
  WriteRegDWORD ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "NoModify" 1
  WriteRegDWORD ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Desktop Shortcut" SEC_DESKTOP
  CreateShortCut "$DESKTOP\${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_EXE}"
SectionEnd

Section "Associate .vx2 files with Neuron Encrypt" SEC_ASSOC
  WriteRegStr HKCR ".vx2" "" "${VX2_CLASS}"
  WriteRegStr HKCR "${VX2_CLASS}" "" "Neuron Encrypt Encrypted File"
  WriteRegStr HKCR "${VX2_CLASS}\DefaultIcon" "" "$INSTDIR\${PRODUCT_EXE},0"
  WriteRegStr HKCR "${VX2_CLASS}\shell\open\command" "" '"$INSTDIR\${PRODUCT_EXE}" "%1"'
  WriteRegDWORD HKLM "Software\NeuronEncrypt" "FileAssocInstalled" 1
  System::Call 'shell32::SHChangeNotify(i 0x8000000, i 0, i 0, i 0)'
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${PRODUCT_EXE}"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk"
  Delete "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall.lnk"
  RMDir "$SMPROGRAMS\${PRODUCT_NAME}"
  Delete "$DESKTOP\${PRODUCT_NAME}.lnk"

  # Improved uninstaller safety: only remove association if we own it
  ReadRegDWORD $0 HKLM "Software\NeuronEncrypt" "FileAssocInstalled"
  ${If} $0 == 1
    ReadRegStr $1 HKCR ".vx2" ""
    ${If} $1 == "${VX2_CLASS}"
      DeleteRegKey HKCR ".vx2"
      DeleteRegKey HKCR "${VX2_CLASS}"
      System::Call 'shell32::SHChangeNotify(i 0x8000000, i 0, i 0, i 0)'
    ${EndIf}
  ${EndIf}

  DeleteRegKey HKLM "Software\NeuronEncrypt"
  DeleteRegKey ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}"
  RMDir "$INSTDIR"
SectionEnd
