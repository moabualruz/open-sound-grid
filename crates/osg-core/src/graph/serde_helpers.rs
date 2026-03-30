//! Custom serde helpers for types that can't use default JSON serialization.
//!
//! `HashMap<K, V>` where K is not a string requires serialization as `Vec<[K, V]>`.

use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};

/// Serialize a HashMap as a JSON array of `[key, value]` pairs.
pub fn serialize_map_as_vec<K, V, S>(map: &HashMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
where
    K: Serialize,
    V: Serialize,
    S: Serializer,
{
    let pairs: Vec<(&K, &V)> = map.iter().collect();
    pairs.serialize(serializer)
}

/// Deserialize a JSON array of `[key, value]` pairs into a HashMap.
pub fn deserialize_map_from_vec<'de, K, V, D>(deserializer: D) -> Result<HashMap<K, V>, D::Error>
where
    K: DeserializeOwned + Eq + Hash,
    V: DeserializeOwned,
    D: Deserializer<'de>,
{
    let pairs: Vec<(K, V)> = Vec::deserialize(deserializer)?;
    Ok(pairs.into_iter().collect())
}
