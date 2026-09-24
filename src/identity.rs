// Immutable identities, with the numeric representation retained for reading history.
use anyhow::{Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;
use uuid::{Uuid, Variant, Version};

pub const LEGACY_MAP: &str = "_legacy-ids.toml";
pub const MIGRATION_JOURNAL: &str = ".identity-migration.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Id {
    Legacy(u32),
    Uuid(Uuid),
}

impl Id {
    pub fn new() -> Result<Self> {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes)
            .map_err(|e| anyhow::anyhow!("cannot generate an item identity: {e}"))?;
        Ok(Self::Uuid(
            uuid::Builder::from_random_bytes(bytes).into_uuid(),
        ))
    }

    pub fn is_uuid(self) -> bool {
        matches!(self, Self::Uuid(_))
    }

    pub fn legacy(self) -> Option<u32> {
        match self {
            Self::Legacy(n) => Some(n),
            Self::Uuid(_) => None,
        }
    }

    pub fn compact(self) -> String {
        match self {
            Self::Legacy(n) => n.to_string(),
            Self::Uuid(id) => id.simple().to_string(),
        }
    }

    pub fn yaml(self) -> serde_yaml_ng::Value {
        match self {
            Self::Legacy(n) => serde_yaml_ng::Value::Number(n.into()),
            Self::Uuid(_) => serde_yaml_ng::Value::String(self.to_string()),
        }
    }
}

impl From<u32> for Id {
    fn from(n: u32) -> Self {
        Self::Legacy(n)
    }
}

impl PartialEq<u32> for Id {
    fn eq(&self, n: &u32) -> bool {
        *self == Self::Legacy(*n)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Legacy(n) => fmt::Display::fmt(n, f),
            Self::Uuid(id) => fmt::Display::fmt(id, f),
        }
    }
}

impl FromStr for Id {
    type Err = anyhow::Error;
    fn from_str(raw: &str) -> Result<Self> {
        let s = raw.trim();
        if s.len() == 32 || s.len() == 36 {
            let id =
                Uuid::parse_str(s).map_err(|_| anyhow::anyhow!("`{raw}` is not a full UUIDv4"))?;
            if id.get_version() != Some(Version::Random) || id.get_variant() != Variant::RFC4122 {
                bail!("`{raw}` is not an RFC 9562 UUIDv4");
            }
            return Ok(Self::Uuid(id));
        }
        s.trim_start_matches('#')
            .parse::<u32>()
            .map(Self::Legacy)
            .map_err(|_| anyhow::anyhow!("`{raw}` is not a valid item id"))
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Legacy(n) => s.serialize_u32(*n),
            Self::Uuid(_) => s.serialize_str(&self.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Input {
            Number(u32),
            Text(String),
        }
        match Input::deserialize(d)? {
            Input::Number(n) => Ok(Self::Legacy(n)),
            Input::Text(s) => s.parse().map_err(serde::de::Error::custom),
        }
    }
}

/// A frozen migration bridge. It is never extended by `new` or import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyMap {
    pub version: u32,
    pub id_format: String,
    pub ids: BTreeMap<String, Id>,
}

impl Default for LegacyMap {
    fn default() -> Self {
        Self {
            version: 1,
            id_format: "{n:04}".into(),
            ids: BTreeMap::new(),
        }
    }
}

impl LegacyMap {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!("unsupported legacy identity map version {}", self.version);
        }
        crate::config::IdFormat::compile(&self.id_format)?;
        let mut targets = BTreeSet::new();
        for (number, id) in &self.ids {
            let n = number.parse::<u32>()?;
            if number != &n.to_string() || !id.is_uuid() || !targets.insert(*id) {
                bail!("invalid or ambiguous legacy identity mapping for {number}");
            }
        }
        Ok(())
    }

    pub fn resolve(&self, raw: &str) -> Option<Id> {
        let number = crate::config::IdFormat::compile(&self.id_format)
            .ok()?
            .read(raw)
            .ok()?;
        self.ids.get(&number.to_string()).copied()
    }

    pub fn canonical(&self, id: Id) -> Id {
        id.legacy()
            .and_then(|n| self.ids.get(&n.to_string()))
            .copied()
            .unwrap_or(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_identities_are_v4_and_round_trip_without_a_counter() {
        let mut ids = BTreeSet::new();
        for _ in 0..1000 {
            let id = Id::new().unwrap();
            assert!(id.is_uuid());
            assert_eq!(id.to_string().parse::<Id>().unwrap(), id);
            assert_eq!(id.compact().parse::<Id>().unwrap(), id);
            assert!(ids.insert(id));
            assert!(serde_json::to_value(id).unwrap().is_string());
        }
    }

    #[test]
    fn legacy_numbers_remain_numbers_and_invalid_uuid_versions_are_refused() {
        assert_eq!(serde_json::to_value(Id::Legacy(67)).unwrap(), 67);
        assert_eq!("67".parse::<Id>().unwrap(), Id::Legacy(67));
        assert!(
            "00000000-0000-7000-8000-000000000000"
                .parse::<Id>()
                .is_err()
        );
        assert!(
            "00000000-0000-4000-0000-000000000000"
                .parse::<Id>()
                .is_err()
        );
        assert!("a83f26b1".parse::<Id>().is_err());
    }
}
