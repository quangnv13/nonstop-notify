!include WinMessages.nsh

!macro NonstopNotifyBroadcastEnvironmentChange
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NonstopNotifyUpdateUserPath action
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\path-integration.ps1" "${action}" "$INSTDIR"'
  Pop $R0
  Pop $R1
  ${If} $R0 != 0
    DetailPrint "Failed to update user PATH: $R1"
    Abort "Failed to update user PATH."
  ${EndIf}
  !insertmacro NonstopNotifyBroadcastEnvironmentChange
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro NonstopNotifyUpdateUserPath "add"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro NonstopNotifyUpdateUserPath "remove"
!macroend
