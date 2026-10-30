//
// AetherFFI.h
// C header for Aether Rust Core native library bindings
//

#ifndef AetherFFI_h
#define AetherFFI_h

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Verifies whether provided token matches expected daemon session token
bool aether_verify_token(const char* expected, const char* provided);

/// Generates a cryptographic session token
char* aether_generate_session_token(void);

/// Formats a wire-ready JSON IPC envelope with framing
char* aether_format_envelope(const char* action, const char* token, const char* params_json);

/// Deallocates a CString returned by the Rust library
void aether_free_string(char* s);

#ifdef __cplusplus
}
#endif

#endif /* AetherFFI_h */
