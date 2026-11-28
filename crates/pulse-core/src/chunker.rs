//! Content-defined chunking and hashing.
//!
//! Files are split with FastCDC so a small edit only changes the chunks around
//! it; each chunk is addressed by its BLAKE3 hash. Peers (M2) transfer only
//! the chunks they are missing.

use std::io::{self, Read};

use fastcdc::v2020::{Error as CdcError, StreamCDC};
use serde::{Deserialize, Serialize};

pub const MIN_CHUNK: usize = 64 * 1024;
pub const AVG_CHUNK: usize = 256 * 1024;
pub const MAX_CHUNK: usize = 1024 * 1024;

/// A 32-byte BLAKE3 hash, hex-encoded when serialized.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    pub fn to_hex(&self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        blake3::Hash::from_hex(s).ok().map(|h| Self(*h.as_bytes()))
    }
}

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hash({})", &self.to_hex()[..12])
    }
}

impl Serialize for Hash {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Hash::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid hash"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub offset: u64,
    pub len: u32,
    pub hash: Hash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDigest {
    /// BLAKE3 of the whole file contents.
    pub hash: Hash,
    pub size: u64,
    pub chunks: Vec<Chunk>,
}

/// Streams `reader` once, producing the whole-file hash and its chunk list.
pub fn digest<R: Read>(reader: R) -> io::Result<FileDigest> {
    let mut file_hasher = blake3::Hasher::new();
    let mut chunks = Vec::new();
    let mut size = 0u64;

    for item in StreamCDC::new(reader, MIN_CHUNK, AVG_CHUNK, MAX_CHUNK) {
        let data = match item {
            Ok(d) => d,
            Err(CdcError::Empty) => break,
            Err(CdcError::IoError(e)) => return Err(e),
            Err(CdcError::Other(msg)) => return Err(io::Error::other(msg)),
        };
        file_hasher.update(&data.data);
        chunks.push(Chunk {
            offset: data.offset,
            len: data.length as u32,
            hash: Hash(*blake3::hash(&data.data).as_bytes()),
        });
        size += data.length as u64;
    }

    Ok(FileDigest {
        hash: Hash(*file_hasher.finalize().as_bytes()),
        size,
        chunks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random bytes (xorshift), so tests need no RNG crate.
    fn noise(len: usize, mut seed: u64) -> Vec<u8> {
        (0..len)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed as u8
            })
            .collect()
    }

    #[test]
    fn empty_input_has_no_chunks() {
        let d = digest(&b""[..]).unwrap();
        assert_eq!(d.size, 0);
        assert!(d.chunks.is_empty());
        assert_eq!(d.hash.0, *blake3::hash(b"").as_bytes());
    }

    #[test]
    fn chunks_cover_the_file_exactly() {
        let data = noise(3 * 1024 * 1024 + 17, 42);
        let d = digest(&data[..]).unwrap();
        assert_eq!(d.size, data.len() as u64);
        assert_eq!(d.hash.0, *blake3::hash(&data).as_bytes());
        let mut expected_offset = 0;
        for c in &d.chunks {
            assert_eq!(c.offset, expected_offset);
            assert!(c.len as usize <= MAX_CHUNK);
            expected_offset += c.len as u64;
        }
        assert_eq!(expected_offset, data.len() as u64);
    }

    #[test]
    fn small_edit_reuses_most_chunks() {
        let original = noise(4 * 1024 * 1024, 7);
        let mut edited = original.clone();
        edited.splice(2_000_000..2_000_000, b"inserted bytes".iter().copied());

        let a = digest(&original[..]).unwrap();
        let b = digest(&edited[..]).unwrap();
        let shared = b
            .chunks
            .iter()
            .filter(|c| a.chunks.iter().any(|o| o.hash == c.hash))
            .count();
        assert!(
            shared + 2 >= b.chunks.len(),
            "only {shared}/{} chunks reused",
            b.chunks.len()
        );
    }

    #[test]
    fn hash_hex_roundtrip() {
        let h = Hash(*blake3::hash(b"pulse").as_bytes());
        assert_eq!(Hash::from_hex(&h.to_hex()), Some(h));
    }
}
