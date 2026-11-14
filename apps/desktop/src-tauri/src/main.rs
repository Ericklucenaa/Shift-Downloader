// Sem janela de console no Windows em builds de release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // O Chrome/Edge chama o executável com a origem da extensão como argumento:
    // nesse caso o SHIFT roda como "native messaging host" (stdin/stdout), sem janela.
    if std::env::args().skip(1).any(|a| a.starts_with("chrome-extension://")) {
        std::process::exit(shift_desktop_lib::native_host::run());
    }
    shift_desktop_lib::run()
}
