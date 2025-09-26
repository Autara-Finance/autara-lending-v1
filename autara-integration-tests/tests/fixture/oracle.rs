use autara_lib::oracle::oracle_config::OracleConfig;

pub fn empty_oracle_config() -> OracleConfig {
    OracleConfig::new_pyth(Default::default(), Default::default())
}
