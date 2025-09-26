use arch_program::pubkey::Pubkey;
use bytemuck::{Pod, Zeroable};

use crate::{
    error::{LendingError, LendingResult},
    padding::Padding,
    pod_option::PodOption,
};

crate::validate_struct!(GlobalConfig, 256, 2);

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, Default)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct GlobalConfig {
    /// Global protocol admin who can manage fees and nominate new admin
    admin: Pubkey,
    /// The nominated admin which can be upgraded to admin
    nominated_admin: PodOption<Pubkey>,
    /// Account which can redeem protocol fees
    fee_receiver: Pubkey,
    /// The share of the protocol fee taken on interest fee
    protocol_fee_share_in_bps: u16,
    pad: Padding<158>,
}

impl GlobalConfig {
    pub fn new(admin: Pubkey, fee_receiver: Pubkey, protocol_fee_share_in_bps: u16) -> Self {
        Self {
            admin,
            fee_receiver,
            protocol_fee_share_in_bps,
            nominated_admin: PodOption::default(),
            pad: Padding::default(),
        }
    }

    pub fn initialize(
        &mut self,
        admin: Pubkey,
        fee_receiver: Pubkey,
        protocol_fee_share_in_bps: u16,
    ) {
        self.admin = admin;
        self.fee_receiver = fee_receiver;
        self.protocol_fee_share_in_bps = protocol_fee_share_in_bps;
    }

    pub fn admin(&self) -> &Pubkey {
        &self.admin
    }

    pub fn fee_receiver(&self) -> &Pubkey {
        &self.fee_receiver
    }

    pub fn can_upgrade_nomination(&self, key: &Pubkey) -> bool {
        self.nominated_admin.as_ref().is_some_and(|n| n == key)
    }

    pub fn upgrade_nomination(&mut self) -> LendingResult {
        if let Some(admin) = self.nominated_admin.take() {
            self.admin = admin;
            Ok(())
        } else {
            return Err(LendingError::InvalidNomination.into());
        }
    }

    pub fn set_fee_receiver(&mut self, fee_receiver: Pubkey) {
        self.fee_receiver = fee_receiver;
    }

    pub fn set_nominated_admin(&mut self, nominated_admin: Pubkey) {
        self.nominated_admin.set(nominated_admin);
    }

    pub fn can_redeem_fees(&self, key: &Pubkey) -> bool {
        &self.admin == key || &self.fee_receiver == key
    }

    pub fn can_update_config(&self, key: &Pubkey) -> bool {
        &self.admin == key || self.nominated_admin.as_ref().is_some_and(|n| n == key)
    }

    pub fn update_protocol_fee_share_in_bps(&mut self, new_fee: u16) {
        self.protocol_fee_share_in_bps = new_fee;
    }

    pub fn protocol_fee_share_in_bps(&self) -> u16 {
        self.protocol_fee_share_in_bps
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use arch_program::pubkey::Pubkey;

    pub fn test_global_config() -> GlobalConfig {
        let admin = Pubkey::new_unique();
        let fee_receiver = Pubkey::new_unique();
        GlobalConfig::new(admin, fee_receiver, 1000)
    }

    #[test]
    fn check_global_config() {
        let admin = Pubkey::new_unique();
        let fee_receiver = Pubkey::new_unique();
        let mut config = GlobalConfig::new(admin, fee_receiver, 1000);

        assert_eq!(config.admin(), &admin);
        assert_eq!(config.fee_receiver(), &fee_receiver);
        assert!(config.can_redeem_fees(&admin));
        assert!(config.can_redeem_fees(&fee_receiver));
        assert!(config.can_update_config(&admin));
        assert!(!config.can_update_config(&fee_receiver));

        config.set_fee_receiver(Pubkey::new_unique());
        config.update_protocol_fee_share_in_bps(1500);

        assert_ne!(config.fee_receiver(), &fee_receiver);
        assert_eq!(config.protocol_fee_share_in_bps(), 1500);

        let nominated_admin = Pubkey::new_unique();
        config.set_nominated_admin(nominated_admin);
        assert!(config.can_update_config(&nominated_admin));
        assert!(!config.can_update_config(&Pubkey::new_unique()));
        assert!(config.can_upgrade_nomination(&nominated_admin));
        assert!(!config.can_upgrade_nomination(&admin));

        config.upgrade_nomination().unwrap();
        assert_eq!(config.admin(), &nominated_admin);
        assert!(!config.can_upgrade_nomination(&nominated_admin));
        assert!(config.can_update_config(&nominated_admin));
        assert!(!config.can_update_config(&admin));
    }
}
