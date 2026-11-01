fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("aetheride.ico");
        res.set("ProductName", "AetherIDE");
        res.set(
            "FileDescription",
            "AetherIDE AI-Native Development Environment",
        );
        res.set("ProductVersion", "0.1.0-beta.2");
        res.set("FileVersion", "0.1.0.2");
        res.set("OriginalFilename", "AetherIDE.exe");
        res.set("InternalName", "AetherIDE");
        res.set(
            "LegalCopyright",
            "Copyright (C) 2026 AetherIDE Project Contributors",
        );
        res.set("CompanyName", "AetherIDE Open Source Project");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resource: {}", e);
        }
    }
}
