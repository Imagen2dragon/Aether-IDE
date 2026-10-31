pub mod auth;
pub mod awareness;
pub mod chunk;
pub mod crdt;
pub mod crypto;
pub mod discovery;
pub mod manifest;
pub mod pairing;

pub use auth::{DeviceIdentity, DeviceRole, RoleRegistry};
pub use awareness::{AwarenessTracker, CursorPosition, PeerPresence, SelectionRange};
pub use chunk::{reassemble_chunks, slice_file, FileChunk, DEFAULT_CHUNK_SIZE};
pub use crdt::{CrdtDocument, CrdtManager};
pub use crypto::{generate_psk, AeadChannel, SyncCryptoError, SyncRole};
pub use discovery::{SyncServiceInfo, AETHER_MDNS_SERVICE_TYPE};
pub use manifest::{diff_manifests, FileEntry, ManifestDiff, WorkspaceManifest};
pub use pairing::PairingPayload;
