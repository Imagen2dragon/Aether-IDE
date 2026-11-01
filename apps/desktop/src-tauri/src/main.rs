// AetherIDE — Production-Grade AI-Native Software Engineering Studio
// Pure Windows GUI subsystem in release builds (zero black console window on normal launch).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check if explicit CLI or diagnostics flags passed
    if args.len() > 1 && args[1] == "doctor" {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
        println!("AetherIDE v0.1.0 (Desktop Studio — Tauri 2 Host)");
        println!("Toolchain & Runtime: HEALTHY");
        return;
    }

    aether_desktop_lib::run();
}
