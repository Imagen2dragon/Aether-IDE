#[cfg(windows)]
use application_core::AetherError;
use application_core::Result;

/// Common abstraction for operating-system pseudoterminal backends.
/// Windows: ConPTY (CreatePseudoConsole / Win32 Job Objects)
/// Linux: POSIX openpty / master-slave PTY
/// macOS: Unix PTY / openpty with zsh/bash lifecycle
pub trait TerminalBackend: Send + Sync {
    fn write_input(&self, data: &[u8]) -> Result<usize>;
    fn read_output(&self, offset: usize) -> (Vec<u8>, usize, bool);
    fn resize(&self, cols: u16, rows: u16) -> Result<()>;
    fn close(&self) -> Result<()>;
    fn pid(&self) -> u32;
    fn is_alive(&self) -> bool;
    fn backend_name(&self) -> &'static str;
}

#[cfg(windows)]
impl TerminalBackend for parking_lot::Mutex<crate::conpty::win32::ConPtySession> {
    fn write_input(&self, data: &[u8]) -> Result<usize> {
        self.lock()
            .write_input(data)
            .map_err(|e| AetherError::Process(e.to_string()))
    }

    fn read_output(&self, offset: usize) -> (Vec<u8>, usize, bool) {
        self.lock().read_output(offset)
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.lock()
            .resize(cols, rows)
            .map_err(|e| AetherError::Process(e.to_string()))
    }

    fn close(&self) -> Result<()> {
        self.lock().close();
        Ok(())
    }

    fn pid(&self) -> u32 {
        self.lock().pid
    }

    fn is_alive(&self) -> bool {
        self.lock().is_alive()
    }

    fn backend_name(&self) -> &'static str {
        "Windows ConPTY"
    }
}
