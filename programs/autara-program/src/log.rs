use arch_program::{log::sol_log_data, msg};

#[inline(never)]
pub fn log(data: &[u8]) {
    msg!("event_start");
    let mut index = 0;
    while index < data.len() {
        let end_index = std::cmp::min(index + autara_lib::event::MAX_EVENT_SIZE, data.len());
        let chunk = &data[index..end_index];
        index = end_index;
        index = end_index;
        sol_log_data(chunk);
    }
    msg!("event_end");
}
