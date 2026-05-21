use adze_glr_core::ParseTable;

pub(super) fn append_large_state_actions(
    compressed_table: &mut Vec<u16>,
    parse_table: &ParseTable,
    large_state_count: usize,
    encode_action: impl Fn(u16) -> u16,
    get_action: impl Fn(usize, usize) -> u16,
) {
    for state in 0..large_state_count {
        for symbol in 0..parse_table.symbol_count {
            let action = get_action(state, symbol);
            compressed_table.push(encode_action(action));
        }
    }
}

pub(super) fn build_small_state_data(
    parse_table: &ParseTable,
    large_state_count: usize,
    encode_action: impl Fn(u16) -> u16,
    get_action: impl Fn(usize, usize) -> u16,
    is_error_action: impl Fn(u16) -> bool,
) -> (Vec<u16>, Vec<u32>) {
    let mut small_table_data = Vec::new();
    let mut small_table_map = Vec::new();

    for state in large_state_count..parse_table.state_count {
        small_table_map.push(small_table_data.len() as u32);

        let mut non_error_actions = Vec::new();
        for symbol in 0..parse_table.symbol_count {
            let action = get_action(state, symbol);
            if !is_error_action(action) {
                non_error_actions.push((symbol, action));
            }
        }

        small_table_data.push(non_error_actions.len() as u16);

        for (symbol, action) in non_error_actions {
            small_table_data.push(symbol as u16);
            small_table_data.push(encode_action(action));
        }
    }

    if small_table_map.is_empty() {
        small_table_map.push(0);
    }

    (small_table_data, small_table_map)
}
