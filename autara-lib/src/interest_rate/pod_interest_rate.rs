use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::{
    interest_rate::{
        curve::{adaptative_curve::AdaptiveInterestRateCurve, polyline::PolylineInterestRateCurve},
        interest_rate_kind::{InterestRateCurveKind, InterestRateKindCurveMut},
        interest_rate_per_second::InterestRatePerSecond,
    },
    math::const_max::const_max_usizes,
};

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Zeroable, BorshSerialize, BorshDeserialize)]
#[borsh(use_discriminant = true)]
pub enum PodInterestRateCurveKind {
    Fixed = 0,
    Polyline = 1,
    Adaptive = 2,
}

impl PodInterestRateCurveKind {
    const fn size(&self) -> usize {
        match self {
            PodInterestRateCurveKind::Fixed => std::mem::size_of::<InterestRatePerSecond>(),
            PodInterestRateCurveKind::Polyline => std::mem::size_of::<PolylineInterestRateCurve>(),
            PodInterestRateCurveKind::Adaptive => std::mem::size_of::<AdaptiveInterestRateCurve>(),
        }
    }
}

unsafe impl Pod for PodInterestRateCurveKind {}

const POD_UNION_SIZE: usize = const_max_usizes(&[
    PodInterestRateCurveKind::Fixed.size(),
    PodInterestRateCurveKind::Polyline.size(),
    PodInterestRateCurveKind::Adaptive.size(),
]);

crate::validate_struct!(PodInterestRateCurve, 72);

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Pod, Zeroable, BorshSerialize, BorshDeserialize)]
pub struct PodInterestRateCurve {
    kind: PodInterestRateCurveKind,
    union: [u8; POD_UNION_SIZE],
}

impl PodInterestRateCurve {
    pub fn from_interest_rate_kind(interest_rate: InterestRateCurveKind) -> Self {
        let mut union = [0; POD_UNION_SIZE];
        match interest_rate {
            InterestRateCurveKind::Fixed(rate) => {
                union[..PodInterestRateCurveKind::Fixed.size()]
                    .copy_from_slice(bytemuck::bytes_of(&rate));
                PodInterestRateCurve {
                    kind: PodInterestRateCurveKind::Fixed,
                    union,
                }
            }
            InterestRateCurveKind::Polyline(poly) => {
                union[..PodInterestRateCurveKind::Polyline.size()]
                    .copy_from_slice(bytemuck::bytes_of(&poly));
                PodInterestRateCurve {
                    kind: PodInterestRateCurveKind::Polyline,
                    union,
                }
            }
            InterestRateCurveKind::Adaptive(adaptive) => {
                union[..PodInterestRateCurveKind::Adaptive.size()]
                    .copy_from_slice(bytemuck::bytes_of(&adaptive));
                PodInterestRateCurve {
                    kind: PodInterestRateCurveKind::Adaptive,
                    union,
                }
            }
        }
    }

    pub fn interest_rate_kind(&self) -> InterestRateCurveKind {
        match self.kind {
            PodInterestRateCurveKind::Fixed => InterestRateCurveKind::Fixed(*bytemuck::from_bytes(
                &self.union[..PodInterestRateCurveKind::Fixed.size()],
            )),
            PodInterestRateCurveKind::Polyline => InterestRateCurveKind::Polyline(
                *bytemuck::from_bytes(&self.union[..PodInterestRateCurveKind::Polyline.size()]),
            ),
            PodInterestRateCurveKind::Adaptive => InterestRateCurveKind::Adaptive(
                *bytemuck::from_bytes(&self.union[..PodInterestRateCurveKind::Adaptive.size()]),
            ),
        }
    }

    pub fn interest_rate_kind_mut(&mut self) -> InterestRateKindCurveMut {
        match self.kind {
            PodInterestRateCurveKind::Fixed => InterestRateKindCurveMut::Fixed(
                bytemuck::from_bytes_mut(&mut self.union[..PodInterestRateCurveKind::Fixed.size()]),
            ),
            PodInterestRateCurveKind::Polyline => {
                InterestRateKindCurveMut::Polyline(bytemuck::from_bytes_mut(
                    &mut self.union[..PodInterestRateCurveKind::Polyline.size()],
                ))
            }
            PodInterestRateCurveKind::Adaptive => {
                InterestRateKindCurveMut::Adaptive(bytemuck::from_bytes_mut(
                    &mut self.union[..PodInterestRateCurveKind::Adaptive.size()],
                ))
            }
        }
    }
}

impl std::fmt::Debug for PodInterestRateCurve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PodInterestRate({:?})", self.interest_rate_kind())
    }
}

impl From<PodInterestRateCurve> for InterestRateCurveKind {
    fn from(pod: PodInterestRateCurve) -> Self {
        pod.interest_rate_kind()
    }
}

impl From<InterestRateCurveKind> for PodInterestRateCurve {
    fn from(interest_rate: InterestRateCurveKind) -> Self {
        PodInterestRateCurve::from_interest_rate_kind(interest_rate)
    }
}

impl Default for PodInterestRateCurve {
    fn default() -> Self {
        PodInterestRateCurve::from_interest_rate_kind(Default::default())
    }
}

#[cfg(feature = "client")]
pub mod serde {
    use crate::interest_rate::interest_rate_kind::InterestRateCurveKind;

    use super::PodInterestRateCurve;

    impl serde::Serialize for PodInterestRateCurve {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            self.interest_rate_kind().serialize(serializer)
        }
    }

    impl<'a> serde::de::Deserialize<'a> for PodInterestRateCurve {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'a>,
        {
            let interest_rate_kind: InterestRateCurveKind =
                InterestRateCurveKind::deserialize(deserializer)?;
            Ok(PodInterestRateCurve::from_interest_rate_kind(
                interest_rate_kind,
            ))
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::interest_rate::curve::{
        adaptative_curve::AdaptiveInterestRateCurve, polyline::PolylineInterestRateCurve,
    };
    use crate::interest_rate::interest_rate_per_second::InterestRatePerSecond;

    #[test]
    fn test_pod_interest_rate_curve() {
        let mut fixed = InterestRatePerSecond(100.into());
        let mut pod_fixed =
            PodInterestRateCurve::from_interest_rate_kind(InterestRateCurveKind::Fixed(fixed));
        assert_eq!(
            pod_fixed.interest_rate_kind(),
            InterestRateCurveKind::Fixed(fixed)
        );
        assert_eq!(
            pod_fixed.interest_rate_kind_mut(),
            InterestRateKindCurveMut::Fixed(&mut fixed)
        );
        let mut polyline =
            PolylineInterestRateCurve::try_new(&[(0, 100).into(), (100, 200).into()]).unwrap();
        let mut pod_polyline = PodInterestRateCurve::from_interest_rate_kind(
            InterestRateCurveKind::Polyline(polyline),
        );
        assert_eq!(
            pod_polyline.interest_rate_kind(),
            InterestRateCurveKind::Polyline(polyline)
        );
        assert_eq!(
            pod_polyline.interest_rate_kind_mut(),
            InterestRateKindCurveMut::Polyline(&mut polyline)
        );
        let mut adaptive = AdaptiveInterestRateCurve::new();
        let mut pod_adaptive = PodInterestRateCurve::from_interest_rate_kind(
            InterestRateCurveKind::Adaptive(adaptive),
        );
        assert_eq!(
            pod_adaptive.interest_rate_kind(),
            InterestRateCurveKind::Adaptive(adaptive)
        );
        assert_eq!(
            pod_adaptive.interest_rate_kind_mut(),
            InterestRateKindCurveMut::Adaptive(&mut adaptive)
        );
    }
}
