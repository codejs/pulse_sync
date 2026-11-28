use std::fmt;
use std::str::FromStr;

use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum IdError {
    #[error("invalid device id")]
    InvalidDeviceId,
    #[error("invalid folder id")]
    InvalidFolderId,
}

/// A device's identity: its Ed25519 public key.
///
/// This is the same key type iroh uses for node ids, so the M2 transport can
/// reuse it directly.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId([u8; 32]);

impl DeviceId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Compact 64-bit id used as the key in version vectors.
    pub fn short(&self) -> u64 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.0[..8]);
        u64::from_be_bytes(b)
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&BASE32_NOPAD.encode(&self.0))
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId({self})")
    }
}

impl FromStr for DeviceId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = BASE32_NOPAD
            .decode(s.to_ascii_uppercase().as_bytes())
            .map_err(|_| IdError::InvalidDeviceId)?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| IdError::InvalidDeviceId)?;
        Ok(Self(arr))
    }
}

impl Serialize for DeviceId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for DeviceId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Identifies a shared folder across all devices. Paths differ per device;
/// the folder id is what peers agree on. Format: `abcde-fgh23`.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FolderId(String);

const FOLDER_ID_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";

impl FolderId {
    pub fn generate() -> Self {
        let mut raw = [0u8; 10];
        getrandom::fill(&mut raw).expect("system RNG unavailable");
        let mut s = String::with_capacity(11);
        for (i, b) in raw.iter().enumerate() {
            if i == 5 {
                s.push('-');
            }
            s.push(FOLDER_ID_ALPHABET[(*b as usize) % 32] as char);
        }
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for FolderId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let valid = (1..=64).contains(&s.len())
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if valid {
            Ok(Self(s.to_owned()))
        } else {
            Err(IdError::InvalidFolderId)
        }
    }
}

impl fmt::Display for FolderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for FolderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FolderId({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_roundtrips_through_string() {
        let id = DeviceId::from_bytes([7u8; 32]);
        let parsed: DeviceId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
        let lower: DeviceId = id.to_string().to_lowercase().parse().unwrap();
        assert_eq!(id, lower);
    }

    #[test]
    fn folder_id_has_expected_shape() {
        let id = FolderId::generate();
        assert_eq!(id.as_str().len(), 11);
        assert_eq!(&id.as_str()[5..6], "-");
        assert!(id.as_str().parse::<FolderId>().is_ok());
        assert!("../etc".parse::<FolderId>().is_err());
    }
}
