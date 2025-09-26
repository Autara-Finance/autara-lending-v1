pub const fn const_max_usizes(items: &[usize]) -> usize {
    let mut max_index = 0;
    let mut index = 0;
    loop {
        if index >= items.len() {
            break;
        }
        let next_item = items[index];
        if next_item > items[max_index] {
            max_index = index;
        }
        index += 1;
    }
    items[max_index]
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_const_max_usizes() {
        let items = [1, 3, 2, 5, 4];
        assert_eq!(const_max_usizes(&items), 5);
    }
}
