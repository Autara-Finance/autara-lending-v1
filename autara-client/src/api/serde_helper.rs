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
