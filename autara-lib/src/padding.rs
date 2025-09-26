use std::fmt::{Debug, Display};

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Pod, Zeroable, BorshSerialize, BorshDeserialize)]
pub struct Padding<const N: usize>(pub [u8; N]);

impl<const N: usize> Debug for Padding<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[_;{}]", N)
    }
}

impl<const N: usize> Display for Padding<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[_;{}]", N)
    }
}

impl<const N: usize> Default for Padding<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

#[cfg(feature = "client")]
pub mod serde {
    use super::Padding;

    impl<const N: usize> serde::Serialize for Padding<N> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            self.to_string().serialize(serializer)
        }
    }

    impl<'de, const N: usize> serde::Deserialize<'de> for Padding<N> {
        fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(Padding([0; N]))
        }
    }
}
