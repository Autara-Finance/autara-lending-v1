use super::math::ifixed_point::IFixedPoint;

pub const DAYS_PER_YEAR: u64 = 365;
pub const SECOND_PER_HOUR: u64 = 60 * 60;
pub const SECONDS_PER_DAY: u64 = 24 * SECOND_PER_HOUR;
pub const SECONDS_PER_YEAR: u64 = DAYS_PER_YEAR * SECONDS_PER_DAY;

pub const MAX_EXPONENT: i64 = 18;

pub const POSITIVE_POWER_OF_TEN: [IFixedPoint; 19] = [
    IFixedPoint::from_i64(1),                   // 10 ^ 0
    IFixedPoint::from_i64(10),                  // 10 ^ 1
    IFixedPoint::from_i64(100),                 // 10 ^ 2
    IFixedPoint::from_i64(1000),                // 10 ^ 3
    IFixedPoint::from_i64(10000),               // 10 ^ 4
    IFixedPoint::from_i64(100000),              // 10 ^ 5
    IFixedPoint::from_i64(1000000),             // 10 ^ 6
    IFixedPoint::from_i64(10000000),            // 10 ^ 7
    IFixedPoint::from_i64(100000000),           // 10 ^ 8
    IFixedPoint::from_i64(1000000000),          // 10 ^ 9
    IFixedPoint::from_i64(10000000000),         // 10 ^ 10
    IFixedPoint::from_i64(100000000000),        // 10 ^ 11
    IFixedPoint::from_i64(1000000000000),       // 10 ^ 12
    IFixedPoint::from_i64(10000000000000),      // 10 ^ 13
    IFixedPoint::from_i64(100000000000000),     // 10 ^ 14
    IFixedPoint::from_i64(1000000000000000),    // 10 ^ 15
    IFixedPoint::from_i64(10000000000000000),   // 10 ^ 16
    IFixedPoint::from_i64(100000000000000000),  // 10 ^ 17
    IFixedPoint::from_i64(1000000000000000000), // 10 ^ 18
];
