//
// TerminalView.swift
// Aether Companion Remote Terminal
//

import SwiftUI

struct TerminalView: View {
    @State private var commandInput = ""
    @State private var terminalLogs = [
        "Aether Remote Terminal Session #1 (PowerShell)",
        "Connected to Windows 11 Desktop (127.0.0.1:44888)",
        "aether-daemon v0.1.0-beta.2 active",
        "$ cargo --version",
        "cargo 1.93.1 (083ac5135 2025-12-15)",
        "$ git status",
        "On branch main, working tree clean",
    ]

    var body: some View {
        NavigationView {
            VStack(spacing: 0) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(terminalLogs, id: \.self) { line in
                            Text(line)
                                .font(.system(.caption, design: .monospaced))
                                .foregroundColor(line.starts(with: "$") ? Color.cyan : Color.white)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                    .padding()
                }
                .background(Color(red: 0.05, green: 0.07, blue: 0.12))

                HStack {
                    Text("$")
                        .foregroundColor(.cyan)
                        .fontWeight(.bold)
                    TextField("Enter command...", text: $commandInput)
                        .font(.system(.body, design: .monospaced))
                        .foregroundColor(.white)
                    Button(action: executeCommand) {
                        Image(systemName: "arrow.up.circle.fill")
                            .font(.title2)
                            .foregroundColor(.cyan)
                    }
                }
                .padding()
                .background(Color(red: 0.08, green: 0.11, blue: 0.18))
            }
            .navigationTitle("Remote Terminal")
        }
    }

    func executeCommand() {
        guard !commandInput.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        terminalLogs.append("$ \(commandInput)")
        terminalLogs.append("[Output] Executing command on Windows workspace...")
        commandInput = ""
    }
}
