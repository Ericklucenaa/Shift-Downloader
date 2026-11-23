; Ganchos do instalador do Shift Downloader (Tauri NSIS).

!macro NSIS_HOOK_PREINSTALL
  ; Até a 0.4.1 o app se chamava "SHIFT" e ficava em %LOCALAPPDATA%\SHIFT. Remove só a instalação antiga
  ; (programa, atalhos e a entrada em "Programas instalados"); dados e configurações ficam em
  ; %APPDATA%\app.shift.downloads e continuam valendo. Não roda o desinstalador antigo: ele apagaria a
  ; inicialização com o Windows e as associações de magnet/.torrent que a pessoa escolheu.
  IfFileExists "$LOCALAPPDATA\SHIFT\shift-desktop.exe" 0 shift_no_old_install
    nsExec::Exec 'taskkill /IM shift-desktop.exe /F'
    nsExec::Exec 'taskkill /IM aria2c.exe /F'
    Sleep 1000
    RMDir /r "$LOCALAPPDATA\SHIFT"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SHIFT"
    Delete "$SMPROGRAMS\SHIFT.lnk"
    Delete "$SMPROGRAMS\SHIFT\SHIFT.lnk"
    RMDir "$SMPROGRAMS\SHIFT"
    Delete "$DESKTOP\SHIFT.lnk"
  shift_no_old_install:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Na instalação silenciosa (/S) não pergunta nada.
  IfSilent shift_skip_ask
  MessageBox MB_YESNO|MB_ICONQUESTION "Usar o SHIFT como programa padrão para links magnet e arquivos .torrent?$\r$\n$\r$\nNa primeira abertura o SHIFT também oferece iniciar junto com o Windows e instalar a extensão do navegador. Tudo pode ser mudado depois em Configurações." IDNO shift_skip_ask
    WriteRegDWORD HKCU "Software\SHIFT" "MakeDefault" 1
  shift_skip_ask:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Remove só o que é do Shift Downloader; associações de outros programas ficam intactas.
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
