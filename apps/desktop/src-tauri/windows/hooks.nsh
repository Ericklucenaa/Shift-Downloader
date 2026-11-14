; Ganchos do instalador do SHIFT (Tauri NSIS).

!macro NSIS_HOOK_POSTINSTALL
  ; Na instalação silenciosa (/S) não pergunta nada.
  IfSilent shift_skip_ask
  MessageBox MB_YESNO|MB_ICONQUESTION "Usar o SHIFT como programa padrão para links magnet e arquivos .torrent?$\r$\n$\r$\nNa primeira abertura o SHIFT também oferece iniciar junto com o Windows e instalar a extensão do navegador. Tudo pode ser mudado depois em Configurações." IDNO shift_skip_ask
    WriteRegDWORD HKCU "Software\SHIFT" "MakeDefault" 1
  shift_skip_ask:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Remove só o que é do SHIFT; associações de outros programas ficam intactas.
  DeleteRegKey HKCU "Software\Classes\SHIFT.magnet"
  DeleteRegKey HKCU "Software\Classes\SHIFT.torrent"
  DeleteRegValue HKCU "Software\Classes\.torrent\OpenWithProgids" "SHIFT.torrent"
  ReadRegStr $0 HKCU "Software\Classes\.torrent" ""
  StrCmp $0 "SHIFT.torrent" 0 +2
    DeleteRegValue HKCU "Software\Classes\.torrent" ""
  ReadRegStr $0 HKCU "Software\Classes\magnet\shell\open\command" ""
  StrCmp $0 '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"' 0 +2
    DeleteRegKey HKCU "Software\Classes\magnet"
  DeleteRegValue HKCU "Software\RegisteredApplications" "SHIFT"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "SHIFT"
  DeleteRegKey HKCU "Software\Google\Chrome\NativeMessagingHosts\app.shift.downloads"
  DeleteRegKey HKCU "Software\Microsoft\Edge\NativeMessagingHosts\app.shift.downloads"
  DeleteRegKey HKCU "Software\SHIFT"
!macroend
