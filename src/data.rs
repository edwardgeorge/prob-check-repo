use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hash {
    Sha1([u8; 20]),
    Sha256([u8; 32]),
}

impl Hash {
    fn sha1_from_bytes(s: &[u8]) -> Result<Self, String> {
        Ok(Self::Sha1(s.try_into().map_err(|e| {
            format!("Hash length should be 32 bits: {e}")
        })?))
    }
    fn sha256_from_bytes(s: &[u8]) -> Result<Self, String> {
        Ok(Self::Sha256(s.try_into().map_err(|e| {
            format!("Hash length should be 32 bits: {e}")
        })?))
    }
}

impl FromStr for Hash {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let l = s.len();
        let d = hex::decode(s).map_err(|e| format!("Could not decode hash from hex '{s}': {e}"))?;
        if l == 40 {
            Self::sha1_from_bytes(&d)
        } else if l == 64 {
            Self::sha256_from_bytes(&d)
        } else {
            Err(format!(
                "Unexpected length of hash '{s}' ({l} chars), expecting 40/64 hex chars"
            ))
        }
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Sha1(s) => &s[..],
            Self::Sha256(s) => &s[..],
        }
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self))
    }
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Status {
    pub check_time: DateTime<Utc>,
    pub change_time: DateTime<Utc>,
    #[serde_as(as = "DisplayFromStr")]
    pub commit_hash: Hash,
}
