; Neuron Encrypt — NSIS Installer Script
; NSIS 3.x with Unicode support
; Builds: makensis installer/neuron-encrypt.nsi

!include "MUI2.nsh"
!include "LogicLib.nsh"

; ---------------------------------------------------------------------------
; General
; ---------------------------------------------------------------------------
!define UNICODE
!define PRODUCT_NAME "Neuron Encrypt"
!define PRODUCT_EXE "neuron-encrypt.exe"
!define PRODUCT_PUBLISHER "Neuron Encrypt Contributors"
!define PRODUCT_VERSION "1.0.0"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\NeuronEncrypt"
!define PRODUCT_UNINST_ROOT_KEY "HKLM"
!define VX2_CLASS "NeuronEncrypt.vx2file"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "NeuronEncrypt-Windows-x64-Setup.exe"
InstallDir "$PROGRAMFILES64\NeuronEncrypt"
InstallDirRegKey ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "InstallLocation"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

; ---------------------------------------------------------------------------
; MUI2 Settings
; ---------------------------------------------------------------------------
!define MUI_ABORTWARNING
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\modern-uninstall.ico"

; Welcome page
!define MUI_WELCOMEPAGE_TITLE "Welcome to ${PRODUCT_NAME} Setup"
!define MUI_WELCOMEPAGE_TEXT "This wizard will guide you through the installation of ${PRODUCT_NAME} ${PRODUCT_VERSION}.$\r$\n$\r$\nAES-256-GCM-SIV file encryption powered by Argon2id and HKDF-SHA512.$\r$\n$\r$\nClick Next to continue."

; Finish page
!define MUI_FINISHPAGE_RUN "$INSTDIR\${PRODUCT_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Launch ${PRODUCT_NAME}"

; ---------------------------------------------------------------------------
; Installer Pages
; ---------------------------------------------------------------------------
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; ---------------------------------------------------------------------------
; Uninstaller Pages
; ---------------------------------------------------------------------------
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; ---------------------------------------------------------------------------
; Language
; ---------------------------------------------------------------------------
!insertmacro MUI_LANGUAGE "English"

; ---------------------------------------------------------------------------
; Installer Sections
; ---------------------------------------------------------------------------
Section "Core Application (required)" SEC_CORE
    SectionIn RO ; Read-only — cannot be deselected

    SetOutPath "$INSTDIR"

    ; Application binary
    File "..\neuron-encrypt\target\x86_64-pc-windows-gnu\release\neuron-encrypt.exe"

    ; License
    File /oname=LICENSE.txt "..\LICENSE"

    ; Create uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; Start Menu shortcut
    CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
    CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_EXE}"
    CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall.lnk" "$INSTDIR\uninstall.exe"

    ; Registry — Add/Remove Programs entry
    WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
    WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${PRODUCT_VERSION}"
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
    ; Register .vx2 extension
    WriteRegStr HKCR ".vx2" "" "${VX2_CLASS}"

    ; Register file type
    WriteRegStr HKCR "${VX2_CLASS}" "" "Neuron Encrypt Encrypted File"
    WriteRegStr HKCR "${VX2_CLASS}\DefaultIcon" "" "$INSTDIR\${PRODUCT_EXE},0"
    WriteRegStr HKCR "${VX2_CLASS}\shell\open\command" "" '"$INSTDIR\${PRODUCT_EXE}" "%1"'

    ; Notify shell of association change
        ; Record that we own the association
    WriteRegDWORD HKLM "Software\NeuronEncrypt" "FileAssocInstalled" 1

    System::Call 'shell32::SHChangeNotify(i 0x8000000, i 0, i 0, i 0)'
SectionEnd

; ---------------------------------------------------------------------------
; Section Descriptions
; ---------------------------------------------------------------------------
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
    !insertmacro MUI_DESCRIPTION_TEXT ${SEC_CORE} "Install the core ${PRODUCT_NAME} application. This component is required."
    !insertmacro MUI_DESCRIPTION_TEXT ${SEC_DESKTOP} "Create a shortcut on your Desktop."
    !insertmacro MUI_DESCRIPTION_TEXT ${SEC_ASSOC} "Associate .vx2 encrypted files with ${PRODUCT_NAME} so they open automatically."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; ---------------------------------------------------------------------------
; Uninstaller
; ---------------------------------------------------------------------------
Section "Uninstall"
    ; Remove application files (only files we installed)
    Delete "$INSTDIR\${PRODUCT_EXE}"
    Delete "$INSTDIR\LICENSE.txt"
    Delete "$INSTDIR\uninstall.exe"

    ; Remove Start Menu shortcuts
    Delete "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk"
    Delete "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall.lnk"
    RMDir "$SMPROGRAMS\${PRODUCT_NAME}"

    ; Remove Desktop shortcut
    Delete "$DESKTOP\${PRODUCT_NAME}.lnk"

    ; Only delete the .vx2 registry keys if we own the association and it still points to us
    ReadRegDWORD $0 HKLM "Software\NeuronEncrypt" "FileAssocInstalled"
    ${If} $0 == 1
        ReadRegStr $1 HKCR ".vx2" ""
        ${If} $1 == "${VX2_CLASS}"
            DeleteRegKey HKCR ".vx2"
            DeleteRegKey HKCR "${VX2_CLASS}"
            System::Call "shell32::SHChangeNotify(i 0x8000000, i 0, i 0, i 0)"
        ${EndIf}
    ${EndIf}

    ; Remove our software key
    DeleteRegKey HKLM "Software\NeuronEncrypt"

    ; Remove Uninstall registry entry
    DeleteRegKey ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}"

    ; Remove install directory ONLY if empty (do NOT delete user .vx2 files)
    RMDir "$INSTDIR"
SectionEnd
