use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::{
    error::LendingResult,
    math::const_max::const_max_usizes,
    oracle::{
        oracle_provider::{
            AccountView, OracleLoader, OracleProvider, OracleProviderRef, UncheckedOracleRate,
        },
        pyth::PythProvider,
    },
};

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Zeroable, BorshSerialize, BorshDeserialize)]
#[borsh(use_discriminant = true)]
pub enum PodOracleProviderKind {
    Pyth = 0,
}

impl PodOracleProviderKind {
    const fn size(&self) -> usize {
        match self {
            PodOracleProviderKind::Pyth => std::mem::size_of::<PythProvider>(),
        }
    }
}

unsafe impl Pod for PodOracleProviderKind {}

const POD_UNION_SIZE: usize = const_max_usizes(&[PodOracleProviderKind::Pyth.size()]);

crate::validate_struct!(PodOracleProvider, 72);

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Pod, Zeroable, BorshSerialize, BorshDeserialize)]
pub struct PodOracleProvider {
    kind: PodOracleProviderKind,
    union: [u8; POD_UNION_SIZE],
}

impl PodOracleProvider {
    pub fn from_oracle_provider(oracle_provider: OracleProvider) -> Self {
        let mut union = [0; POD_UNION_SIZE];
        match oracle_provider {
            OracleProvider::Pyth(pyth_provider) => {
                union[..PodOracleProviderKind::Pyth.size()]
                    .copy_from_slice(bytemuck::bytes_of(&pyth_provider));
                PodOracleProvider {
                    kind: PodOracleProviderKind::Pyth,
                    union,
                }
            }
        }
    }

    pub fn oracle_provider(&self) -> OracleProvider {
        match self.kind {
            PodOracleProviderKind::Pyth => OracleProvider::Pyth(*bytemuck::from_bytes(
                &self.union[..PodOracleProviderKind::Pyth.size()],
            )),
        }
    }

    pub fn oracle_provider_ref(&self) -> OracleProviderRef {
        match self.kind {
            PodOracleProviderKind::Pyth => OracleProviderRef::Pyth(bytemuck::from_bytes(
                &self.union[..PodOracleProviderKind::Pyth.size()],
            )),
        }
    }
}

impl std::fmt::Debug for PodOracleProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PodOracleProvider({:?})", self.oracle_provider())
    }
}

impl From<PodOracleProvider> for OracleProvider {
    fn from(pod: PodOracleProvider) -> Self {
        pod.oracle_provider()
    }
}

impl From<OracleProvider> for PodOracleProvider {
    fn from(oracle_provider: OracleProvider) -> Self {
        PodOracleProvider::from_oracle_provider(oracle_provider)
    }
}

impl OracleLoader for PodOracleProvider {
    fn load_oracle_price<D: std::ops::Deref<Target = [u8]>>(
        &self,
        view: AccountView<D>,
    ) -> LendingResult<UncheckedOracleRate> {
        self.oracle_provider_ref().load_oracle_price(view)
    }
}

#[cfg(feature = "client")]
pub mod serde {
    use crate::oracle::oracle_provider::OracleProvider;

    use super::PodOracleProvider;

    impl serde::Serialize for PodOracleProvider {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            self.oracle_provider().serialize(serializer)
        }
    }

    impl<'a> serde::de::Deserialize<'a> for PodOracleProvider {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'a>,
        {
            let oracle_provider: OracleProvider = OracleProvider::deserialize(deserializer)?;
            Ok(PodOracleProvider::from_oracle_provider(oracle_provider))
        }
    }
}
