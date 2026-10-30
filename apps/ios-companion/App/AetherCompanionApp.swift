//
// AetherCompanionApp.swift
// AetherIDE iOS/iPadOS Native Companion
//

import SwiftUI

@main
struct AetherCompanionApp: App {
    @StateObject private var connectionManager = ConnectionManager()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(connectionManager)
                .preferredColorScheme(.dark)
        }
    }
}
