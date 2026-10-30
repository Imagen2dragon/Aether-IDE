//
// AgentsView.swift
// Aether Companion AI Agent Monitor & Control
//

import SwiftUI

struct AgentsView: View {
    @State private var taskObjective = ""
    @State private var selectedRole = "architect"
    @State private var selectedMode = "Assist"

    let roles = [
        "architect": "Principal Architect",
        "frontend": "Frontend Developer",
        "backend": "Backend Developer",
        "database": "Database Engineer",
        "mobile": "Mobile Developer",
        "systems": "Rust/C++ Engineer",
        "debugging": "Debugging Specialist",
        "testing": "Testing Specialist",
        "security": "Security Reviewer"
    ]

    var body: some View {
        NavigationView {
            VStack {
                Form {
                    Section(header: Text("Delegate New Autonomous Task")) {
                        Picker("Role", selection: $selectedRole) {
                            ForEach(roles.keys.sorted(), id: \.self) { key in
                                Text(roles[key]!).tag(key)
                            }
                        }

                        Picker("Autonomy Mode", selection: $selectedMode) {
                            Text("Observe").tag("Observe")
                            Text("Assist").tag("Assist")
                            Text("Autopilot").tag("Autopilot")
                        }

                        TextField("Objective / Prompt", text: $taskObjective)

                        Button(action: dispatchTask) {
                            HStack {
                                Spacer()
                                Text("Dispatch to Desktop IDE")
                                    .fontWeight(.bold)
                                    .foregroundColor(.white)
                                Spacer()
                            }
                        }
                        .listRowBackground(Color(red: 0.14, green: 0.44, blue: 0.88))
                    }

                    Section(header: Text("Change Approval Requests")) {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Step: Modify cargo dependencies")
                                .font(.subheadline)
                                .fontWeight(.bold)
                            Text("+ reqwest = { version = \"0.12\", features = [\"json\"] }")
                                .font(.system(.caption, design: .monospaced))
                                .foregroundColor(.green)
                            HStack {
                                Button("Reject") {}
                                    .foregroundColor(.red)
                                Spacer()
                                Button("Approve Step") {}
                                    .foregroundColor(.green)
                            }
                            .padding(.top, 4)
                        }
                    }
                }
            }
            .navigationTitle("AI Agents")
        }
    }

    func dispatchTask() {
        taskObjective = ""
    }
}
