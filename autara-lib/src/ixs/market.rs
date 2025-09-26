use crate::{
    interest_rate::interest_rate_kind::InterestRateCurveKind, math::ifixed_point::IFixedPoint,
    oracle::oracle_config::OracleConfig, pda::find_market_pda, state::market_config::LtvConfig,
    token::get_associated_token_address,
};
use arch_program::{account::AccountMeta, instruction::Instruction, pubkey::Pubkey};
use borsh::{BorshDeserialize, BorshSerialize};

use super::types::AurataInstruction;
use crate::pda::find_global_config_pda;

#[repr(C)]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct CreateMarketInstruction {
    pub market_bump: u8,
    pub index: u8,
    pub ltv_config: LtvConfig,
    pub max_utilisation_rate: IFixedPoint,
    pub supply_oracle_config: OracleConfig,
    pub collateral_oracle_config: OracleConfig,
    pub interest_rate: InterestRateCurveKind,
    pub lending_market_fee_in_bps: u16,
}

#[repr(C)]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct UpdateConfigInstruction {
    #[cfg_attr(feature = "client", serde(default))]
    pub supply_oracle_config: Option<OracleConfig>,
    #[cfg_attr(feature = "client", serde(default))]
    pub collateral_oracle_config: Option<OracleConfig>,
    #[cfg_attr(feature = "client", serde(default))]
    pub ltv_config: Option<LtvConfig>,
    #[cfg_attr(feature = "client", serde(default))]
    pub max_supply_atoms: Option<u64>,
    #[cfg_attr(feature = "client", serde(default))]
    pub max_utilisation_rate: Option<IFixedPoint>,
    #[cfg_attr(feature = "client", serde(default))]
    pub lending_market_fee_in_bps: Option<u16>,
}

pub fn create_market_ix(
    mut create_market: CreateMarketInstruction,
    supply_mint: Pubkey,
    collateral_mint: Pubkey,
    autara_program_id: Pubkey,
    curator: Pubkey,
    payer: Pubkey,
) -> (Pubkey, Instruction) {
    let mut data = Vec::new();
    let (market_pda, market_bump) = find_market_pda(
        &autara_program_id,
        &curator,
        &supply_mint,
        &collateral_mint,
        create_market.index,
    );
    create_market.market_bump = market_bump;
    let ix = AurataInstruction::CreateMarket(create_market);
    ix.serialize(&mut data).unwrap();
    let accounts = vec![
        AccountMeta::new_readonly(curator, true),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(find_global_config_pda(&autara_program_id).0, false),
        AccountMeta::new(market_pda, false),
        AccountMeta::new_readonly(supply_mint, false),
        AccountMeta::new(
            get_associated_token_address(&market_pda, &supply_mint),
            false,
        ),
        AccountMeta::new_readonly(collateral_mint, false),
        AccountMeta::new(
            get_associated_token_address(&market_pda, &collateral_mint),
            false,
        ),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(apl_associated_token_account::id(), false),
        AccountMeta::new_readonly(arch_program::system_program::SYSTEM_PROGRAM_ID, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    (
        market_pda,
        Instruction {
            program_id: autara_program_id,
            accounts,
            data,
        },
    )
}

pub fn update_config_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    curator: Pubkey,
    config: UpdateConfigInstruction,
    supply_oracle_key: Pubkey,
    collateral_oracle_key: Pubkey,
) -> Instruction {
    let supply_oracle = if let Some(oracle) = &config.supply_oracle_config {
        oracle
            .oracle_provider()
            .oracle_provider_ref()
            .autara_pyth_pubkey()
            .unwrap()
    } else {
        supply_oracle_key
    };
    let collateral_oracle = if let Some(oracle) = &config.collateral_oracle_config {
        oracle
            .oracle_provider()
            .oracle_provider_ref()
            .autara_pyth_pubkey()
            .unwrap()
    } else {
        collateral_oracle_key
    };
    let mut data = Vec::new();
    AurataInstruction::UpdateConfig(config)
        .serialize(&mut data)
        .unwrap();

    let accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new_readonly(find_global_config_pda(&autara_program_id).0, false),
        AccountMeta::new_readonly(curator, true),
        AccountMeta::new_readonly(supply_oracle, false),
        AccountMeta::new_readonly(collateral_oracle, false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}

pub fn reedeem_curator_fees_ix(
    autara_program_id: Pubkey,
    market: Pubkey,
    curator: Pubkey,
    curator_supply_ata: Pubkey,
    market_supply_vault: Pubkey,
) -> Instruction {
    let mut data = Vec::new();
    AurataInstruction::ReedeemCuratorFees
        .serialize(&mut data)
        .unwrap();
    let accounts = vec![
        AccountMeta::new_readonly(curator, true),
        AccountMeta::new(market, false),
        AccountMeta::new(curator_supply_ata, false),
        AccountMeta::new(market_supply_vault, false),
        AccountMeta::new_readonly(apl_token::id(), false),
        AccountMeta::new_readonly(autara_program_id, false),
    ];
    Instruction {
        program_id: autara_program_id,
        accounts,
        data,
    }
}
