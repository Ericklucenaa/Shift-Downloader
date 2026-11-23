//! Suspender, hibernar ou desligar depois que todos os downloads terminam.
//! Programas do sistema chamados diretamente, com argumentos fixos (sem shell montado).

pub fn run(action: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let (program, args): (&str, &[&str]) = match action {
            // SetSuspendState via .NET: suspende de verdade mesmo com a hibernação ligada.
            "sleep" => (
                "powershell.exe",
                &["-NoProfile", "-NonInteractive", "-Command", "Add-Type -AssemblyName System.Windows.Forms; [void][System.Windows.Forms.Application]::SetSuspendState('Suspend', $false, $false)"],
            ),
            "hibernate" => ("shutdown.exe", &["/h"]),
            // 30 s de folga: `shutdown /a` ainda cancela.
            "shutdown" => ("shutdown.exe", &["/s", "/t", "30", "/c", "Shift Downloader: todos os downloads terminaram."]),
            _ => return Err(std::io::Error::other("ação desconhecida")),
        };
        std::process::Command::new(program).args(args).creation_flags(CREATE_NO_WINDOW).spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        Err(std::io::Error::other("disponível só no Windows por enquanto"))
    }
}
