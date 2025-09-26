#[macro_export]
macro_rules! assert_struct_size {
    ($struct_name:ty, $size:expr) => {
        const _: () = {
            assert!(std::mem::size_of::<$struct_name>() == $size);
        };
    };
}

#[macro_export]
macro_rules! assert_struct_alignment {
    ($struct_name:ty, $alignment:expr) => {
        const _: () = {
            assert!(std::mem::align_of::<$struct_name>() == $alignment);
        };
    };
}

#[macro_export]
macro_rules! validate_struct {
    ($struct_name:ty, $size:expr, $alignment:expr) => {
        $crate::assert_struct_size!($struct_name, $size);
        $crate::assert_struct_alignment!($struct_name, $alignment);
    };
    ($struct_name:ty, $size:expr) => {
        $crate::validate_struct!($struct_name, $size, 8);
    };
}
