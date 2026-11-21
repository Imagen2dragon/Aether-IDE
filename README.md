# AetherIDE

A Windows desktop IDE for editing, debugging, and building mobile projects. Aether Obsidian is the default workbench theme. Aether Assistant and Mobile Studio ship with the preview.

## Aether Assistant

Open **Aether Assistant** with **Ctrl+Alt+I** or search for it in the Command Palette. Start Ollama on this computer and install a model to enable chat. Choose a local model and, optionally, one of the 14 Aether Engineering Team specialists. Replies stream into the panel. The assistant uses the conversation you enter and the specialist instructions you select.

## Mobile Studio

- **Android** — Open a project with a Gradle wrapper. Mobile Studio runs the project's Gradle tasks to generate a debug APK or release bundle. Device discovery uses ADB.
- **iOS** — Mobile Studio provides xtool build and device commands. Supply a working xtool environment with the project's Apple signing setup. On Windows the integration uses WSL. A build to a physical iPhone remains to be verified.

## Build the Windows preview

From this source directory, run the build and installer in PowerShell.

```powershell
./scripts/aether/dev.ps1 -Task Install
./scripts/aether/dev.ps1 -Task Package
./scripts/aether/install-preview.ps1
```

Open **AetherIDE Preview** from the Start menu after installation. See the [migration notes](AETHER_MIGRATION.md) for the build and validation history.

AetherIDE builds on [Code - OSS](https://github.com/microsoft/vscode) and includes its [MIT license](LICENSE.txt) and [third-party notices](ThirdPartyNotices.txt).
