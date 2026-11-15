# Windows Desktop Manual-Test Checklist

Use this checklist against a packaged Windows desktop build (not only the browser preview). Record the app version, Windows version, installer/package type, and pass/fail result for each item. For failures, attach the relevant app log and a screenshot, while redacting usernames, private paths, and file contents.

## Test setup

- [ ] Install or extract a fresh build on a supported Windows machine and launch `AetherIDE.exe` normally.
- [ ] Confirm the window opens without a blank screen, Rust panic, missing runtime, or startup dialog error.
- [ ] Confirm the title bar identifies the desktop runtime accurately and does not claim engine health unless a real health probe was run.
- [ ] Prepare a temporary test workspace with a nested directory, a text file, a source file, a filename containing spaces, and a Unicode filename. Keep a second workspace for switching tests.
- [ ] Record the app build/version, Windows edition/build, and whether Windows PowerShell or PowerShell 7 is installed.

## Native Open Folder dialog

- [ ] Choose **File → Open Folder** (and test the keyboard shortcut if one is advertised). A Windows-native folder picker appears, owned by the app window.
- [ ] Navigate to the prepared workspace, select it, and confirm. The dialog closes and the selected workspace name/path appears in the UI.
- [ ] Verify Explorer populates with the actual directory contents, not sample/demo entries; expand and collapse nested directories.
- [ ] Open a different workspace and verify the Explorer tree, Git state, terminal working directory, and workspace label update to the new root.
- [ ] Cancel the folder dialog. The current workspace, tabs, and editor contents remain unchanged; no false “opened” notification appears.
- [ ] Open a path containing spaces and non-ASCII characters. Confirm the displayed path and Explorer contents remain correct.
- [ ] Attempt to pass a nonexistent folder path through any supported CLI/deep-link entry point. The app reports a useful error and does not claim success.
- [ ] Verify repeated dialog use works (open, cancel, open again) without COM initialization errors, a frozen UI, or a leaked dialog.

## Open/add file and save workflows

- [ ] Use **File → Open File** with a file inside the current workspace. Confirm the correct file opens in an editor tab and its text is exact.
- [ ] Open a file outside the current workspace. Confirm its full path is retained and that saving it writes to the selected file—not an unrelated same-named workspace file.
- [ ] Cancel the Open File dialog. No blank/placeholder tab is created and no cancellation error is shown as a failure.
- [ ] Open a file whose name collides with another folder’s file. Confirm the tabs and title show unambiguous paths or otherwise distinguish the files.
- [ ] Edit a file and save with **Ctrl+S**. Reopen it from disk (or verify with another editor) and confirm the exact text persisted.
- [ ] Use **Save As** to create a new file in another directory, including a path with spaces. Confirm the dialog respects the chosen path and the new file exists there.
- [ ] Attempt to save to a read-only or invalid destination. The UI reports the failure and does not mark the tab clean.
- [ ] Modify a file, then open a different workspace. Verify the unsaved-changes confirmation offers a safe choice; canceling preserves the current workspace and dirty buffer.
- [ ] Close a dirty tab and confirm the user is not silently allowed to lose changes.
- [ ] Check that binary/unreadable files fail clearly rather than opening as fabricated text.

## Explorer operations

- [ ] Refresh Explorer after creating or renaming a test file externally. The actual filesystem state appears.
- [ ] Create a file and a directory using Explorer actions. Confirm each item exists on disk and appears in the correct location.
- [ ] Open files from nested folders. Confirm they open using the selected root and full path.
- [ ] If multi-root is supported, add a second root, verify both roots are separately displayed, and confirm opening files in either root works.
- [ ] If rename/delete actions are shown, verify they affect only the selected item, handle cancellation, and refresh the tree. Do not test destructive actions on important files.
- [ ] Verify an empty workspace displays a genuine empty state, not a fabricated project tree.

## ConPTY terminal

- [ ] Open the integrated Terminal. A native shell process is created; no hard-coded prompt or simulated “Executed:” response is presented as a real shell.
- [ ] Run `Get-Location` (PowerShell) and verify the current directory is the active workspace root. For a workspace path with spaces, verify it is not truncated.
- [ ] Run `Write-Output 'Aether terminal test'` and verify actual output arrives in the terminal.
- [ ] Run a harmless filesystem command such as `Get-ChildItem` and verify results correspond to the selected workspace.
- [ ] Run a command that produces both standard output and an error, for example `Write-Output 'stdout'; Write-Error 'stderr test'`. Verify both streams are visible and ordering is reasonable.
- [ ] Enter a long command, edit it with Backspace/arrows, and submit it. Confirm keystrokes and terminal echo come from the actual shell.
- [ ] Run a short command that waits briefly, then use **Ctrl+C**. Confirm it is interrupted and the shell remains usable.
- [ ] Resize the panel and the app window. Confirm the terminal reflows and remains interactive without duplicated output or corrupted characters.
- [ ] Create two terminal sessions if supported. Confirm input/output and working directories remain isolated per session.
- [ ] Close a terminal session/panel and confirm its process exits. Reopen Terminal and verify a fresh session starts.
- [ ] Close the app while a shell is active. Confirm child shell/ConPTY resources are cleaned up and do not remain orphaned.
- [ ] Start the app where the native terminal backend/daemon is deliberately unavailable (test environment only). Confirm a clear failure/retry message appears and command input is not falsely echoed as executed.
- [ ] Test with the configured default shell unavailable or misconfigured. The app reports the actual startup error and does not show a fake PowerShell session.

## Other workbench panels and runtime status

- [ ] Open Problems, Output, Debug Console, Tests, Doctor, and Ports. Empty or unsupported panels state that no results/session are available; they do not show fabricated diagnostics, LSP logs, test passes, or subsystem health.
- [ ] Confirm the Rust/desktop status badge describes runtime presence only unless a real backend health check has completed.
- [ ] If the app advertises F5/debug, testing, or language-server features, exercise each advertised action and confirm it connects to a real backend or reports that the feature is unavailable.

## Native dialog edge cases

- [ ] Open/cancel/reopen folder and file dialogs repeatedly; verify no hang or crash.
- [ ] Test directory and file names containing spaces, parentheses, ampersands, and Unicode characters.
- [ ] Test a removable/network location if supported; cancellation or an inaccessible location should yield a useful error.
- [ ] Verify dialog cancellation is treated as cancellation, not as an error or empty successful workspace.
- [ ] Confirm dialogs stay in front of the parent app and return focus to it when closed.

## Exit criteria

- [ ] All critical flows—Open Folder, Explorer population, Open File, Save, native dialog cancellation, and a real ConPTY shell—pass on a clean packaged build.
- [ ] No fabricated terminal commands, health results, test results, or engine-online claims remain in the tested build.
- [ ] Record all failures with reproduction steps and logs. Do not mark Windows-specific behavior verified based only on a frontend build.