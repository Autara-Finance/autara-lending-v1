pub mod serde_pubkey {
    use arch_sdk::arch_program::pubkey::Pubkey;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(pubkey: &Pubkey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = hex::encode(pubkey.as_ref());
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Pubkey::from_slice(
            &hex::decode(s).map_err(serde::de::Error::custom)?,
        ))
    }
}

pub mod serde_pubkey_vec {
    use arch_sdk::arch_program::pubkey::Pubkey;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(keys: &[Pubkey], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_keys: Vec<String> = keys.iter().map(|k| hex::encode(k.as_ref())).collect();
        hex_keys.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Pubkey>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex_keys: Vec<String> = Vec::deserialize(deserializer)?;
        hex_keys
            .into_iter()
            .map(|s| {
                let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
                Ok(Pubkey::from_slice(&bytes))
            })
            .collect()
    }
}

pub mod serde_optional_pubkey {
    use arch_sdk::arch_program::pubkey::Pubkey;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(pubkey: &Option<Pubkey>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match pubkey {
            Some(pk) => serializer.serialize_str(&hex::encode(pk.as_ref())),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Pubkey>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Option::<String>::deserialize(deserializer)?;
        match s {
            Some(s) => {
                let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
                Ok(Some(Pubkey::from_slice(&bytes)))
            }
            None => Ok(None),
        }
    }
}

pub mod serde_from_str {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: ToString,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

pub mod serde_from_optional_str {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S, T>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: ToString,
    {
        match value {
            Some(v) => serializer.serialize_str(&v.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        let s = Option::<String>::deserialize(deserializer)?;
        match s {
            Some(s) => s.parse().map_err(serde::de::Error::custom).map(Some),
            None => Ok(None),
        }
    }
}
