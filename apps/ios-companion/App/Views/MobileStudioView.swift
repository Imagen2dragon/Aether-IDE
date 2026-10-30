//
// MobileStudioView.swift
// Aether Companion Mobile App Studio & EAS Preview
//

import SwiftUI

struct MobileStudioView: View {
    @State private var appName = "AetherCommerceApp"
    @State private var buildPlatform = "ios"

    var body: some View {
        NavigationView {
            Form {
                Section(header: Text("Connected Mobile Workspace")) {
                    Text("App: \(appName)")
                    Text("Framework: React Native + Expo")
                    Text("Status: EAS Ready")
                }

                Section(header: Text("Trigger Remote Cloud Build")) {
                    Picker("Target Platform", selection: $buildPlatform) {
                        Text("iOS (TestFlight / IPA)").tag("ios")
                        Text("Android (APK / AAB)").tag("android")
                    }

                    Button(action: triggerBuild) {
                        HStack {
                            Spacer()
                            Text("Start EAS Build Job")
                                .fontWeight(.bold)
                                .foregroundColor(.white)
                            Spacer()
                        }
                    }
                    .listRowBackground(Color.blue)
                }
            }
            .navigationTitle("Mobile Studio")
        }
    }

    func triggerBuild() {}
}

class ConnectionManager: ObservableObject {
    @Published var isConnected = true
    @Published var isPairingRequired = false
    @Published var pairedHost = "127.0.0.1:44888"
}

struct PairingView: View {
    @Environment(\.presentationMode) var presentationMode

    var body: some View {
        VStack(spacing: 20) {
            Text("Pair with Windows IDE")
                .font(.title2)
                .fontWeight(.bold)
            Text("Scan the QR code displayed in AetherIDE settings to pair this companion device over an end-to-end encrypted channel.")
                .font(.body)
                .multilineTextAlignment(.center)
                .padding()

            RoundedRectangle(cornerRadius: 16)
                .stroke(Color.cyan, lineWidth: 2)
                .frame(width: 220, height: 220)
                .overlay(
                    Image(systemName: "qrcode.viewfinder")
                        .font(.system(size: 64))
                        .foregroundColor(.cyan)
                )

            Button("Cancel") {
                presentationMode.wrappedValue.dismiss()
            }
            .foregroundColor(.gray)
        }
        .padding()
    }
}
