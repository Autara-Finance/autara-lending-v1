pub mod borrow_position;
pub mod collateral_vault;
pub mod global_config;
pub mod market;
pub mod market_config;
pub mod market_wrapper;
pub mod supply_position;
pub mod supply_vault;

// Autara Lending Accounts are discriminated by their size.
const _: () = const {
    let accounts_size = [
        size_of::<borrow_position::BorrowPosition>(),
        size_of::<supply_position::SupplyPosition>(),
        size_of::<market::Market>(),
        size_of::<global_config::GlobalConfig>(),
    ];
    validate_all_different_sizes(accounts_size);
};

#[allow(dead_code)]
const fn validate_all_different_sizes<const M: usize>(sizes: [usize; M]) {
    let mut i = 0;
    while i < sizes.len() {
        let mut j = i + 1;
        while j < sizes.len() {
            if sizes[i] == sizes[j] {
                panic!("duplicated size");
            }
            j += 1;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_all_different_sizes() {
        validate_all_different_sizes([1, 2, 3, 4]);
    }

    #[test]
    #[should_panic(expected = "duplicated size")]
    fn test_validate_all_different_sizes_fail() {
        validate_all_different_sizes([2, 2, 4, 3]);
    }
}
