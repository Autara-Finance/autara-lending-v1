use std::fmt::Debug;

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct PodOption<T: Pod + Zeroable + PartialEq>(T);

unsafe impl<T: Pod + Zeroable + PartialEq> Pod for PodOption<T> {}
unsafe impl<T: Pod + Zeroable + PartialEq> Zeroable for PodOption<T> {}

impl<T: Pod + Zeroable + PartialEq> PodOption<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn take(&mut self) -> Option<T> {
        if self.0 == T::zeroed() {
            None
        } else {
            let value = std::mem::replace(&mut self.0, T::zeroed());
            Some(value)
        }
    }

    pub fn as_ref(&self) -> Option<&T> {
        if self.0 == T::zeroed() {
            None
        } else {
            Some(&self.0)
        }
    }

    pub fn set(&mut self, value: T) {
        self.0 = value;
    }
}

impl<T: Pod + Zeroable + PartialEq + Debug> Debug for PodOption<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}
