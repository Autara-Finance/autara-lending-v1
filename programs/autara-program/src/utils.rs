use arch_program::clock::Clock;

pub fn clock() -> Clock {
    let mut clock = Clock::default();
    unsafe { arch_program::syscalls::arch_get_clock(&mut clock) };
    if clock.unix_timestamp == 0 {
        panic!()
    }
    clock
}
