pub const FLOAT_RELATIVE_EPSILON: f64 = 1e-6;

#[macro_export]
macro_rules! assert_eq_float {
    ($left:expr, $right:expr, $max_relative_error:expr) => {{
        let left_val = $left;
        let right_val = $right;
        let abs_relative_error = (left_val - right_val).abs() / right_val;
        if abs_relative_error > $max_relative_error {
            panic!(
                "assertion failed: `(left == right)`\n  left: `{:?}`,\n right: `{:?}`,\n max_relative_error: `{}`,\n abs_relative_error: `{:?}`",
                left_val, right_val, $max_relative_error, abs_relative_error
            );
        }
    }};
    ($left:expr, $right:expr) => {
        $crate::assert_eq_float!($left, $right, $crate::math::test_utils::FLOAT_RELATIVE_EPSILON)
    };
}
