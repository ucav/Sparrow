!define APPNAME "Sparrow"
!define COMPANY "ucav"
!define DESCRIPTION "Sparrow"
!define VERSION "0.9.0"

Name "${APPNAME}"
OutFile "Sparrow-Setup.exe"
InstallDir "$LOCALAPPDATA\Sparrow"
RequestExecutionLevel user

Page directory
Page instfiles

Section "Install"
  SetOutPath "$INSTDIR\bin"
  File "..\..\target\release\sparrow.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortcut "$DESKTOP\Sparrow.lnk" "$INSTDIR\bin\sparrow.exe" "" "$INSTDIR\bin\sparrow.exe" 0
  CreateDirectory "$SMPROGRAMS\Sparrow"
  CreateShortcut "$SMPROGRAMS\Sparrow\Sparrow.lnk" "$INSTDIR\bin\sparrow.exe"
  EnVar::SetHKCU
  EnVar::AddValue "Path" "$INSTDIR\bin"
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\Sparrow.lnk"
  Delete "$SMPROGRAMS\Sparrow\Sparrow.lnk"
  RMDir "$SMPROGRAMS\Sparrow"
  Delete "$INSTDIR\bin\sparrow.exe"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir /r "$INSTDIR"
SectionEnd
