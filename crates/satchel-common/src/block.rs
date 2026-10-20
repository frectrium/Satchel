//! Content-addressed block identity.
//!
//! Blocks in Satchel are immutable and keyed by the hash of their contents, so
//! identical data is stored once. Magic Pocket keys blocks by SHA-256; Satchel
//! does the same. Record any change to this in the Design Decision Log.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// SHA-256 digest of a block's contents.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct BlockHash([u8; 32]);

impl BlockHash {
    /// Hashes `data` to produce its content address.
    pub fn of(data: &[u8]) -> Self {
        Self(Sha256::digest(data).into())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Returns true if `data` hashes to this address.
    pub fn verify(&self, data: &[u8]) -> bool {
        Self::of(data) == *self
    }
}

impl fmt::Display for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl fmt::Debug for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockHash({self})")
    }
}

impl FromStr for BlockHash {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s, &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl From<BlockHash> for String {
    fn from(hash: BlockHash) -> Self {
        hash.to_string()
    }
}

impl TryFrom<String> for BlockHash {
    type Error = hex::FromHexError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_data_has_identical_address() {
        assert_eq!(BlockHash::of(b"satchel"), BlockHash::of(b"satchel"));
        assert_ne!(BlockHash::of(b"satchel"), BlockHash::of(b"pocket"));
    }

    #[test]
    fn matches_known_sha256_vector() {
        assert_eq!(
            BlockHash::of(b"abc").to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn round_trips_through_hex() {
        let hash = BlockHash::of(b"round trip");
        assert_eq!(hash.to_string().parse::<BlockHash>().unwrap(), hash);
    }

    #[test]
    fn verify_detects_corruption() {
        let hash = BlockHash::of(b"intact");
        assert!(hash.verify(b"intact"));
        assert!(!hash.verify(b"corrupt"));
    }
}
