//
// AetherBridge.swift
// Native Swift binding bridge to Aether Rust Core via FFI & TCP/WebSocket Transport
//

import Foundation

public final class AetherBridge: ObservableObject {
    public static let shared = AetherBridge()

    @Published public var isConnected: Bool = false
    @Published public var host: String = "127.0.0.1"
    @Published public var port: UInt16 = 44888
    @Published public var sessionToken: String = ""
    @Published public var statusMessage: String = "Disconnected"

    private init() {}

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
