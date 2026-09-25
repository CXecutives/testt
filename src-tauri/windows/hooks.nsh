; Installer hooks of the Windows setup (tauri.windows.conf.json > bundle > windows > nsis >
; installerHooks). After every install the shortcuts name the app's own icon explicitly and the
; shell is told to reload its icons: a shortcut without an icon path (",0") could keep a stale
; or blank picture from the icon cache after an update.

!macro NSIS_HOOK_POSTINSTALL
  IfFileExists "$DESKTOP\${PRODUCTNAME}.lnk" 0 +2
    CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
  IfFileExists "$SMPROGRAMS\${PRODUCTNAME}.lnk" 0 +2
    CreateShortCut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
  ; SHCNE_ASSOCCHANGED: the shell drops its cached icons and reads them again.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
