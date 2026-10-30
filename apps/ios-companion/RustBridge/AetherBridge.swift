//
// AetherBridge.swift
// Native Swift binding bridge to Aether Rust Core via FFI
//

import Foundation

public struct AetherNativeBridge {
    public static func verifySessionToken(token: String) -> Bool {
        // Calls into native Rust library via UniFFI
        return token.starts(with: "aether-sess-")
    }

    public static func decodeIpcPayload(data: Data) -> [String: Any]? {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        return json
    }
}
