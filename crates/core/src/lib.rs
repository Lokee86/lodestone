mod error;
mod format;
mod materialize;
mod materialize_format;
mod object;
mod object_format;
#[cfg(all(target_endian = "little", any(unix, windows)))]
mod packed_store;
mod search;
mod snapshot;
mod transport;

#[cfg(all(test, target_endian = "little", any(unix, windows)))]
mod benchmark;

pub use error::{Error, Result};
pub use lodestone_packed::{PackedVectorError, PackedVectors, ScalarType, VectorSchema};
pub use materialize::materialize;
pub use object::ObjectStore;
#[cfg(all(target_endian = "little", any(unix, windows)))]
pub use packed_store::{
    PackedGeneration, PackedReclamation, PackedRecord, PackedStore, PackedUpdate,
};
pub use search::{PackedCosineHit, SearchHit, search, search_packed_cosine};
pub use snapshot::{RecordRef, Snapshot, SnapshotInfo};
pub use transport::{ingest_jsonl, materialize_jsonl};
