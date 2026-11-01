; AetherIDE NSIS Installer Hooks
; Configures user-level desktop shortcuts, Start Menu integration, App Paths (Win+R "aetheride" / "aether"),
; and clean uninstallation without requiring UAC elevation.

!macro NSIS_HOOK_POSTINSTALL
  ; Register App Paths so running "aetheride" or "aether" from Win+R or Command Prompt works immediately
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\AetherIDE.exe" "" "$INSTDIR\AetherIDE.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\AetherIDE.exe" "Path" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aetheride.exe" "" "$INSTDIR\AetherIDE.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aetheride.exe" "Path" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aether.exe" "" "$INSTDIR\AetherIDE.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aether.exe" "Path" "$INSTDIR"

  ; Application description and registration
  WriteRegStr HKCU "Software\Classes\Applications\AetherIDE.exe" "FriendlyAppName" "AetherIDE"
  WriteRegStr HKCU "Software\Classes\Applications\AetherIDE.exe\DefaultIcon" "" "$INSTDIR\AetherIDE.exe,0"
  WriteRegStr HKCU "Software\Classes\Applications\AetherIDE.exe\shell\open\command" "" '"$INSTDIR\AetherIDE.exe" "%1"'

  ; Create Desktop and Start Menu Shortcuts
  CreateShortCut "$DESKTOP\AetherIDE.lnk" "$INSTDIR\AetherIDE.exe" "" "$INSTDIR\AetherIDE.exe" 0 SW_SHOWNORMAL "" "AetherIDE — AI-Native Engineering Studio"
  CreateShortCut "$SMPROGRAMS\AetherIDE.lnk" "$INSTDIR\AetherIDE.exe" "" "$INSTDIR\AetherIDE.exe" 0 SW_SHOWNORMAL "" "AetherIDE — AI-Native Engineering Studio"

  ; Notify Windows Explorer of shell changes
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Remove Desktop and Start Menu Shortcuts
  Delete "$DESKTOP\AetherIDE.lnk"
  Delete "$SMPROGRAMS\AetherIDE.lnk"

  ; Clean up App Paths registry keys
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\AetherIDE.exe"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aetheride.exe"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\aether.exe"
  DeleteRegKey HKCU "Software\Classes\Applications\AetherIDE.exe"

  ; Refresh Windows Explorer
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
