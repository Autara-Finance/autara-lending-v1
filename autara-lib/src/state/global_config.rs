use arch_program::pubkey::Pubkey;
use bytemuck::{Pod, Zeroable};

use crate::{
    error::{LendingError, LendingResult},
    math::bps::ONE_IN_BPS,
    padding::Padding,
    pod_option::PodOption,
};

/// The protocol share is taken out of the lending fee, so it can never exceed 100%.
/// A larger share would make the curator remainder negative when the fee is split.
pub const MAX_PROTOCOL_FEE_SHARE_IN_BPS: u16 = ONE_IN_BPS as u16;

fn validate_protocol_fee_share_in_bps(protocol_fee_share_in_bps: u16) -> LendingResult {
    if protocol_fee_share_in_bps > MAX_PROTOCOL_FEE_SHARE_IN_BPS {
        return Err(LendingError::FeeTooHigh.into());
    }
    Ok(())
}

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
    ) -> LendingResult {
        validate_protocol_fee_share_in_bps(protocol_fee_share_in_bps)?;
        self.admin = admin;
        self.fee_receiver = fee_receiver;
        self.protocol_fee_share_in_bps = protocol_fee_share_in_bps;
        Ok(())
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
        &self.admin == key
    }

    pub fn update_protocol_fee_share_in_bps(&mut self, new_fee: u16) -> LendingResult {
        validate_protocol_fee_share_in_bps(new_fee)?;
        self.protocol_fee_share_in_bps = new_fee;
        Ok(())
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
        config.update_protocol_fee_share_in_bps(1500).unwrap();

        assert_ne!(config.fee_receiver(), &fee_receiver);
        assert_eq!(config.protocol_fee_share_in_bps(), 1500);

        // Fee above 100% (10_000 bps) should fail
        assert!(config.update_protocol_fee_share_in_bps(10_001).is_err());
        assert_eq!(config.protocol_fee_share_in_bps(), 1500); // unchanged

        let nominated_admin = Pubkey::new_unique();
        config.set_nominated_admin(nominated_admin);
        // Nominated admin should NOT have config privileges before accepting
        assert!(!config.can_update_config(&nominated_admin));
        assert!(!config.can_update_config(&Pubkey::new_unique()));
        // But can upgrade nomination
        assert!(config.can_upgrade_nomination(&nominated_admin));
        assert!(!config.can_upgrade_nomination(&admin));

        config.upgrade_nomination().unwrap();
        assert_eq!(config.admin(), &nominated_admin);
        assert!(!config.can_upgrade_nomination(&nominated_admin));
        assert!(config.can_update_config(&nominated_admin));
        assert!(!config.can_update_config(&admin));
    }

    #[test]
    fn initialize_rejects_protocol_fee_share_above_one_hundred_percent() {
        let mut config = GlobalConfig::default();
        assert_eq!(
            config
                .initialize(
                    Pubkey::new_unique(),
                    Pubkey::new_unique(),
                    MAX_PROTOCOL_FEE_SHARE_IN_BPS + 1,
                )
                .unwrap_err(),
            LendingError::FeeTooHigh
        );
        assert_eq!(config.protocol_fee_share_in_bps(), 0);
    }

    #[test]
    fn initialize_accepts_the_full_share() {
        let mut config = GlobalConfig::default();
        config
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                MAX_PROTOCOL_FEE_SHARE_IN_BPS,
            )
            .unwrap();
        assert_eq!(
            config.protocol_fee_share_in_bps(),
            MAX_PROTOCOL_FEE_SHARE_IN_BPS
        );
    }

    #[test]
    fn initialize_and_update_share_the_same_bound() {
        let too_high = MAX_PROTOCOL_FEE_SHARE_IN_BPS + 1;
        let mut initialized = GlobalConfig::default();
        assert!(initialized
            .initialize(Pubkey::new_unique(), Pubkey::new_unique(), too_high)
            .is_err());
        let mut updated = test_global_config();
        assert!(updated.update_protocol_fee_share_in_bps(too_high).is_err());
    }
}
