use arch_sdk::AccountFilter;

pub fn market_filter() -> Vec<AccountFilter> {
    vec![AccountFilter::DataSize(std::mem::size_of::<
        autara_lib::state::market::Market,
    >())]
}

pub fn supply_position_filter() -> Vec<AccountFilter> {
    vec![AccountFilter::DataSize(std::mem::size_of::<
        autara_lib::state::supply_position::SupplyPosition,
    >())]
}

pub fn borrow_position_filter() -> Vec<AccountFilter> {
    vec![AccountFilter::DataSize(std::mem::size_of::<
        autara_lib::state::borrow_position::BorrowPosition,
    >())]
}

pub fn global_config_filter() -> Vec<AccountFilter> {
    vec![AccountFilter::DataSize(std::mem::size_of::<
        autara_lib::state::global_config::GlobalConfig,
    >())]
}
