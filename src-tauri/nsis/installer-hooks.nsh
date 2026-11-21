; Tauri uses the NSIS Modern UI "Show readme" checkbox as its
; "Create a desktop shortcut" option on the finish page.
; Keep the option available, but leave it unchecked by default.
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED

; Offer a normal, opt-in setup step after the resources have been installed.
; The helper does the same hardware checks and backup as the manual tool.
; Never prompt or elevate from silent, passive, or automatic updater runs.
!define MUI_FINISHPAGE_REBOOTLATER_DEFAULT
Var PanelModesPowerShell
Var PanelModesResult
Var PanelModesMessage

!macro NSIS_HOOK_POSTINSTALL
  ${IfNot} ${Silent}
  ${AndIf} $PassiveMode != 1
  ${AndIf} $UpdateMode != 1
    ; Tauri's NSIS installer is 32-bit; Sysnative starts 64-bit PowerShell.
    StrCpy $PanelModesPowerShell "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
    nsExec::ExecToStack '"$PanelModesPowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\panel-setup\Installer-PanelModes.ps1" -Action Probe'
    Pop $PanelModesResult
    Pop $PanelModesMessage
    ${If} $PanelModesResult == 0
      MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "Add optional 48Hz and 120Hz panel modes?$\r$\n$\r$\nUseful for movie watching and frame sync on this supported Yoga Pro 9i panel.$\r$\n$\r$\nSetup will check the requirements and request administrator access. A Windows restart is needed afterward. Your current refresh rate will stay the same.$\r$\n$\r$\nChoose No to install the app without adding panel modes." IDNO panel_modes_done
      DetailPrint "Setting up optional 48Hz and 120Hz panel modes..."
      nsExec::ExecToStack '"$PanelModesPowerShell" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\panel-setup\Installer-PanelModes.ps1" -Action Install'
      Pop $PanelModesResult
      Pop $PanelModesMessage
      ${If} $PanelModesResult == 3010
        SetRebootFlag true
        DetailPrint "48Hz and 120Hz panel modes installed. Restart Windows to make them available."
      ${ElseIf} $PanelModesResult == 10
        DetailPrint "48Hz and 120Hz panel modes are already installed."
      ${ElseIf} $PanelModesResult == 1223
        DetailPrint "Panel setup skipped: administrator request cancelled. The app is installed."
      ${ElseIf} $PanelModesResult == 20
        ; The elevated helper already showed the specific reason.
        DetailPrint "Panel setup could not finish. The app is installed; panel modes can be added later."
      ${Else}
        MessageBox MB_OK|MB_ICONEXCLAMATION "The app was installed, but the optional panel modes could not be added.$\r$\n$\r$\n$PanelModesMessage$\r$\n$\r$\nYou can run setup again later."
      ${EndIf}
    ${ElseIf} $PanelModesResult == 10
      DetailPrint "48Hz and 120Hz panel modes are already installed."
    ${ElseIf} $PanelModesResult == 3010
      SetRebootFlag true
      DetailPrint "48Hz and 120Hz panel modes are already set up. Restart Windows to make them available."
    ${Else}
      DetailPrint "Optional panel modes were not changed: $PanelModesMessage"
    ${EndIf}
    panel_modes_done:
  ${EndIf}
!macroend

