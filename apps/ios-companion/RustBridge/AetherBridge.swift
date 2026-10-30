//
// AetherBridge.swift
// Native Swift binding bridge to Aether Rust Core via FFI & TCP/WebSocket Transport
//

import Foundation
import Network

// MARK: - Workspace Synchronization Models

public struct PairingSession: Codable, Equatable {
    public let host: String
    public let port: UInt16
    public let sessionId: String
    public let pskHex: String
    public let expiresAt: UInt64

    public var isExpired: Bool {
        let now = UInt64(Date().timeIntervalSince1970)
        return now > expiresAt
    }

    public static func parsePairingUri(_ uriString: String) -> PairingSession? {
        guard uriString.hasPrefix("aether://pair?"),
              let components = URLComponents(string: uriString),
              let queryItems = components.queryItems else {
            return nil
        }

        var host: String?
        var port: UInt16?
        var sessionId: String?
        var pskHex: String?
        var expiresAt: UInt64?

        for item in queryItems {
            switch item.name {
            case "host": host = item.value
            case "port": if let v = item.value, let p = UInt16(v) { port = p }
            case "session": sessionId = item.value
            case "key": pskHex = item.value
            case "exp": if let v = item.value, let e = UInt64(v) { expiresAt = e }
            default: break
            }
        }

        guard let h = host, let p = port, let s = sessionId, let k = pskHex, let exp = expiresAt else {
            return nil
        }

        return PairingSession(host: h, port: p, sessionId: s, pskHex: k, expiresAt: exp)
    }
}

public struct SyncFileEntry: Codable, Equatable {
    public let path: String
    public let size: UInt64
    public let sha256: String
    public let modifiedAt: UInt64

    enum CodingKeys: String, CodingKey {
        case path
        case size
        case sha256
        case modifiedAt = "modified_at"
    }
}

public struct SyncWorkspaceManifest: Codable, Equatable {
    public let workspaceName: String
    public let rootHash: String
    public let entries: [String: SyncFileEntry]
    public let generatedAt: UInt64

    enum CodingKeys: String, CodingKey {
        case workspaceName = "workspace_name"
        case rootHash = "root_hash"
        case entries
        case generatedAt = "generated_at"
    }
}

public struct SyncManifestDiff: Codable, Equatable {
    public let toUpload: [String]
    public let toDownload: [String]
    public let toDelete: [String]
    public let conflicts: [String]

    enum CodingKeys: String, CodingKey {
        case toUpload = "to_upload"
        case toDownload = "to_download"
        case toDelete = "to_delete"
        case conflicts
    }
}

public final class AetherBridge: ObservableObject {
    public static let shared = AetherBridge()

    @Published public var isConnected: Bool = false
    @Published public var host: String = "127.0.0.1"
    @Published public var port: UInt16 = 44888
    @Published public var sessionToken: String = ""
    @Published public var statusMessage: String = "Disconnected"
    @Published public var activePairing: PairingSession? = nil

    private init() {}

    public func pairWithUri(_ uri: String) -> Bool {
        guard let session = PairingSession.parsePairingUri(uri) else {
            self.statusMessage = "Invalid QR code URI"
            return false
        }
        if session.isExpired {
            self.statusMessage = "Pairing session expired"
            return false
        }
        self.activePairing = session
        self.host = session.host
        self.port = session.port
        self.sessionToken = session.pskHex
        self.statusMessage = "Paired with \(session.host):\(session.port)"
        return true
    }

    public func computeDiff(local: SyncWorkspaceManifest, remote: SyncWorkspaceManifest) -> SyncManifestDiff {
        var toUpload: [String] = []
        var toDownload: [String] = []
        var conflicts: [String] = []

        for (path, localEntry) in local.entries {
            if let remoteEntry = remote.entries[path] {
                if localEntry.sha256 != remoteEntry.sha256 {
                    if localEntry.modifiedAt != remoteEntry.modifiedAt {
                        conflicts.append(path)
                    } else {
                        toUpload.append(path)
                    }
                }
            } else {
                toUpload.append(path)
            }
        }

        for (path, _) in remote.entries where local.entries[path] == nil {
            toDownload.append(path)
        }

        return SyncManifestDiff(
            toUpload: toUpload,
            toDownload: toDownload,
            toDelete: [],
            conflicts: conflicts
        )
    }

    public func verifySessionToken(expected: String, provided: String) -> Bool {
        #if canImport(AetherFFI)
        return expected.withCString { expPtr in
            provided.withCString { provPtr in
                aether_verify_token(expPtr, provPtr)
            }
        }
        #else
        return expected == provided && !provided.isEmpty
        #endif
    }

    public func formatEnvelope(action: String, params: [String: Any] = [:]) -> String? {
        let paramsJson = (try? JSONSerialization.data(withJSONObject: params))
            .flatMap { String(data: $0, encoding: .utf8) } ?? "{}"

        #if canImport(AetherFFI)
        return action.withCString { actPtr in
            sessionToken.withCString { tokPtr in
                paramsJson.withCString { parPtr in
                    guard let cStr = aether_format_envelope(actPtr, tokPtr, parPtr) else {
                        return nil
                    }
                    let res = String(cString: cStr)
                    aether_free_string(cStr)
                    return res
                }
            }
        }
        #else
        let envelope: [String: Any] = [
            "id": "ios-\(UUID().uuidString)",
            "session_token": sessionToken.isEmpty ? NSNull() : sessionToken,
            "type": "Request",
            "data": [
                "action": action,
                "params": params
            ]
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: envelope),
              let json = String(data: data, encoding: .utf8) else {
            return nil
        }
        return json + "\n"
        #endif
    }

    @MainActor
    public func connect(host: String = "127.0.0.1", port: UInt16 = 44888, token: String) async -> Bool {
        self.host = host
        self.port = port
        self.sessionToken = token
        self.statusMessage = "Connecting to \(host):\(port)..."

        // Perform TCP ping check
        guard let envelope = formatEnvelope(action: "Ping") else {
            self.statusMessage = "Failed to format handshake envelope"
            return false
        }

        do {
            let resp = try await sendRawRequest(envelope: envelope)
            self.isConnected = true
            self.statusMessage = "Connected to Aether Daemon v0.1.0"
            return true
        } catch {
            self.statusMessage = "Connection failed: \(error.localizedDescription)"
            self.isConnected = false
            return false
        }
    }

    public func sendRawRequest(envelope: String) async throws -> String {
        return try await withCheckedThrowingContinuation { continuation in
            let connection = NWConnection(
                host: NWEndpoint.Host(self.host),
                port: NWEndpoint.Port(rawValue: self.port)!,
                using: .tcp
            )

            connection.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    guard let data = envelope.data(using: .utf8) else {
                        continuation.resume(throwing: NSError(domain: "AetherBridge", code: -1))
                        return
                    }
                    connection.send(content: data, completion: .contentProcessed({ error in
                        if let error = error {
                            continuation.resume(throwing: error)
                            return
                        }
                        connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { data, _, _, error in
                            if let error = error {
                                continuation.resume(throwing: error)
                            } else if let data = data, let respStr = String(data: data, encoding: .utf8) {
                                continuation.resume(returning: respStr)
                            } else {
                                continuation.resume(returning: "")
                            }
                            connection.cancel()
                        }
                    }))
                case .failed(let err):
                    continuation.resume(throwing: err)
                default:
                    break
                }
            }

            connection.start(queue: .global())
        }
    }
}
