pub use pod_i128::PodI128;
pub use pod_u128::PodU128;

mod pod_i128 {
    crate::define_pod_int!(PodI128, i128, fixed::types::I80F48);
}
mod pod_u128 {
    crate::define_pod_int!(PodU128, u128, fixed::types::U64F64);
}
