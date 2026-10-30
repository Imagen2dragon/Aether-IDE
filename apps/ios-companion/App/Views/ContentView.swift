//
// ContentView.swift
// AetherIDE iOS Companion Main Navigation
//

import SwiftUI

struct ContentView: View {
    @EnvironmentObject var connection: ConnectionManager
    @State private var selectedTab = 0

    var body: some View {
        TabView(selection: $selectedTab) {
            HomeView()
                .tabItem {
                    Label("Home", systemImage: "house.fill")
                }
                .tag(0)

            AgentsView()
                .tabItem {
                    Label("AI Agents", systemImage: "bolt.fill")
                }
                .tag(1)

            RemoteCodeView()
                .tabItem {
                    Label("Code", systemImage: "chevron.left.forwardslash.chevron.right")
                }
                .tag(2)

            TerminalView()
                .tabItem {
                    Label("Terminal", systemImage: "terminal.fill")
                }
                .tag(3)

            MobileStudioView()
                .tabItem {
                    Label("Mobile App", systemImage: "iphone")
                }
                .tag(4)

            BuildCenterView()
                .tabItem {
                    Label("Builds", systemImage: "hammer.fill")
                }
                .tag(5)

            SettingsView()
                .tabItem {
                    Label("Settings", systemImage: "gearshape.fill")
                }
                .tag(6)
        }
        .accentColor(Color(red: 0.22, green: 0.74, blue: 0.97)) // Aether cyan
        .sheet(isPresented: $connection.isPairingRequired) {
            PairingView()
        }
    }
}

struct HomeView: View {
    @EnvironmentObject var connection: ConnectionManager
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    // Desktop IDE Status Banner
                    HStack {
                        VStack(alignment: .leading) {
                            Text("Windows Desktop IDE")
                                .font(.headline)
                                .foregroundColor(.white)
                            Text(connection.isConnected ? "Paired & Streaming" : "Searching for IDE...")
                                .font(.subheadline)
                                .foregroundColor(connection.isConnected ? .green : .yellow)
                        }
                        Spacer()
                        Circle()
                            .fill(connection.isConnected ? Color.green : Color.yellow)
                            .frame(width: 12, height: 12)
                    }
                    .padding()
                    .background(Color(red: 0.08, green: 0.12, blue: 0.20))
                    .cornerRadius(12)

                    Text("Active Engineering Agents")
                        .font(.title3)
                        .fontWeight(.bold)
                        .foregroundColor(.white)

                    AgentCardView(role: "Principal Architect", status: "Analyzing Dependencies", objective: "Change Impact Radar scan on IPC schema")
                    AgentCardView(role: "Debugging Specialist", status: "Reproduction Lab", objective: "Investigate socket timeout trace")

                    Text("Recent EAS Cloud Builds")
                        .font(.title3)
                        .fontWeight(.bold)
                        .foregroundColor(.white)

                    BuildItemView(app: "AetherCommerceApp", platform: "iOS (TestFlight)", status: "Completed", time: "12m ago")
                    BuildItemView(app: "AetherCommerceApp", platform: "Android (APK)", status: "Building", time: "Just now")
                }
                .padding()
            }
            .navigationTitle("Aether Companion")
            .background(Color(red: 0.04, green: 0.06, blue: 0.10).edgesIgnoringSafeArea(.all))
        }
    }
}

struct AgentCardView: View {
    let role: String
    let status: String
    let objective: String

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(role)
                    .font(.caption)
                    .fontWeight(.bold)
                    .foregroundColor(Color(red: 0.22, green: 0.74, blue: 0.97))
                Spacer()
                Text(status)
                    .font(.caption2)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Color.blue.opacity(0.2))
                    .cornerRadius(6)
                    .foregroundColor(.blue)
            }
            Text(objective)
                .font(.body)
                .foregroundColor(.white)
        }
        .padding()
        .background(Color(red: 0.07, green: 0.10, blue: 0.16))
        .cornerRadius(10)
    }
}

struct BuildItemView: View {
    let app: String
    let platform: String
    let status: String
    let time: String

    var body: some View {
        HStack {
            VStack(alignment: .leading) {
                Text(app)
                    .font(.subheadline)
                    .fontWeight(.semibold)
                    .foregroundColor(.white)
                Text(platform)
                    .font(.caption)
                    .foregroundColor(.gray)
            }
            Spacer()
            VStack(alignment: .trailing) {
                Text(status)
                    .font(.caption)
                    .foregroundColor(status == "Completed" ? .green : .orange)
                Text(time)
                    .font(.caption2)
                    .foregroundColor(.gray)
            }
        }
        .padding()
        .background(Color(red: 0.07, green: 0.10, blue: 0.16))
        .cornerRadius(10)
    }
}

struct RemoteCodeView: View {
    var body: some View {
        NavigationView {
            Text("Remote Code Explorer & Diff Review")
                .foregroundColor(.white)
                .navigationTitle("Code")
                .background(Color(red: 0.04, green: 0.06, blue: 0.10).edgesIgnoringSafeArea(.all))
        }
    }
}

struct BuildCenterView: View {
    var body: some View {
        NavigationView {
            Text("EAS & Cloud Build Center")
                .foregroundColor(.white)
                .navigationTitle("Builds")
                .background(Color(red: 0.04, green: 0.06, blue: 0.10).edgesIgnoringSafeArea(.all))
        }
    }
}

struct SettingsView: View {
    var body: some View {
        NavigationView {
            Text("Settings & Device Pairing")
                .foregroundColor(.white)
                .navigationTitle("Settings")
                .background(Color(red: 0.04, green: 0.06, blue: 0.10).edgesIgnoringSafeArea(.all))
        }
    }
}
