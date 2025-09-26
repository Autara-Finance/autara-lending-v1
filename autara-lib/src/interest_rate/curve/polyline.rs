use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::{
    interest_rate::interest_rate_per_second::InterestRatePerSecond,
    math::{bps::ONE_IN_BPS, ifixed_point::IFixedPoint, safe_math::SafeMath},
};

pub const POLYLINE_MAX_POINTS: usize = 8;

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Pod, Zeroable, BorshSerialize, BorshDeserialize)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct PolylineInterestRateCurve {
    points: [PolylineInterestRatePoint; POLYLINE_MAX_POINTS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolylineInterestRateCurveError {
    TooManyPoints,
    FirstPointInvalid,
    PointsNotInOrder,
}

impl PolylineInterestRateCurve {
    pub fn try_new(
        points: &[PolylineInterestRatePoint],
    ) -> Result<Self, PolylineInterestRateCurveError> {
        Self::validate_points(points)?;
        let mut points_array = [PolylineInterestRatePoint {
            utilization_rate_bps: 0,
            borrow_rate_bps: 0,
        }; POLYLINE_MAX_POINTS];
        for (i, point) in points.iter().enumerate() {
            points_array[i] = *point;
        }
        Ok(Self {
            points: points_array,
        })
    }

    pub fn validate(&self) -> Result<(), PolylineInterestRateCurveError> {
        Self::validate_points(&self.points)
    }

    pub fn points(&self) -> impl Iterator<Item = &PolylineInterestRatePoint> {
        self.points.iter().map_while(|point| point.maybe_uninit())
    }

    pub fn interest_rate_per_second(&self, utilization_rate_bps: u32) -> InterestRatePerSecond {
        InterestRatePerSecond::from_apr(
            IFixedPoint::from_num(self.apr_borrow_rate_bps(utilization_rate_bps))
                .safe_div(ONE_IN_BPS as u64)
                .expect("should be non zero"),
        )
    }

    pub fn apr_borrow_rate_bps(&self, utilization_rate_bps: u32) -> u32 {
        let mut points = self.points();
        let mut start = points.next().expect("At least one point must exist");
        let mut end = match points.next() {
            Some(next) => next,
            None => {
                return start.borrow_rate_bps;
            }
        };
        for point in points {
            if utilization_rate_bps < end.utilization_rate_bps {
                break;
            }
            start = end;
            end = point;
        }
        Line { start, end }.value_at(utilization_rate_bps)
    }

    fn validate_points(
        points: &[PolylineInterestRatePoint],
    ) -> Result<(), PolylineInterestRateCurveError> {
        if points.len() > POLYLINE_MAX_POINTS {
            return Err(PolylineInterestRateCurveError::TooManyPoints);
        }
        let mut reached_end = false;
        let first = points
            .first()
            .ok_or(PolylineInterestRateCurveError::FirstPointInvalid)?;
        if first.utilization_rate_bps != 0 || first.borrow_rate_bps == 0 {
            return Err(PolylineInterestRateCurveError::FirstPointInvalid);
        }
        let mut last_borrow_rate_bps = first.borrow_rate_bps;
        let mut last_utilization_rate_bps = 0;
        for maybe_uninit in points.iter().map(|p| p.maybe_uninit()).skip(1) {
            match maybe_uninit {
                Some(point) => {
                    if reached_end
                        || point.utilization_rate_bps <= last_utilization_rate_bps
                        || point.borrow_rate_bps <= last_borrow_rate_bps
                    {
                        return Err(PolylineInterestRateCurveError::PointsNotInOrder);
                    }

                    last_borrow_rate_bps = point.borrow_rate_bps;
                    last_utilization_rate_bps = point.utilization_rate_bps;
                }
                None => reached_end = true,
            }
        }
        Ok(())
    }
}

impl Default for PolylineInterestRateCurve {
    fn default() -> Self {
        let mut points = [PolylineInterestRatePoint {
            utilization_rate_bps: 0,
            borrow_rate_bps: 0,
        }; POLYLINE_MAX_POINTS];
        points[0] = PolylineInterestRatePoint {
            utilization_rate_bps: 0,
            borrow_rate_bps: 2_00,
        };
        points[1] = PolylineInterestRatePoint {
            utilization_rate_bps: 92_00,
            borrow_rate_bps: 7_00,
        };
        points[2] = PolylineInterestRatePoint {
            utilization_rate_bps: 100_00,
            borrow_rate_bps: 100_00,
        };
        Self { points }
    }
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Pod, Zeroable, BorshSerialize, BorshDeserialize)]
#[cfg_attr(
    feature = "client",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct PolylineInterestRatePoint {
    utilization_rate_bps: u32,
    borrow_rate_bps: u32,
}

impl PolylineInterestRatePoint {
    pub fn maybe_uninit(&self) -> Option<&Self> {
        if self.borrow_rate_bps == 0 {
            None
        } else {
            Some(self)
        }
    }
}

impl From<(u32, u32)> for PolylineInterestRatePoint {
    fn from((utilization_rate_bps, borrow_rate_bps): (u32, u32)) -> Self {
        PolylineInterestRatePoint {
            utilization_rate_bps,
            borrow_rate_bps,
        }
    }
}

struct Line<'a> {
    start: &'a PolylineInterestRatePoint,
    end: &'a PolylineInterestRatePoint,
}

impl<'a> Line<'a> {
    /// caller should assure that start and end are not the same point
    /// and that utilization_rate_bps should be greater than or equal to start.utilization_rate_bps
    fn value_at(&self, utilization_rate_bps: u32) -> u32 {
        self.start.borrow_rate_bps.saturating_add(
            ((self.end.borrow_rate_bps as u64 - self.start.borrow_rate_bps as u64)
                * (utilization_rate_bps as u64 - self.start.utilization_rate_bps as u64)
                / (self.end.utilization_rate_bps as u64 - self.start.utilization_rate_bps as u64))
                as u32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_default() {
        PolylineInterestRateCurve::default()
            .validate()
            .expect("Default curve should be valid");
    }

    #[test]
    fn test_line_value_at() {
        let start = PolylineInterestRatePoint {
            utilization_rate_bps: 1000, // 10%
            borrow_rate_bps: 200,       // 2%
        };
        let end = PolylineInterestRatePoint {
            utilization_rate_bps: 5000, // 50%
            borrow_rate_bps: 1000,      // 10%
        };

        let line = Line {
            start: &start,
            end: &end,
        };

        assert_eq!(line.value_at(1000), 200);
        assert_eq!(line.value_at(3000), 600);
        assert_eq!(line.value_at(5000), 1000);
        assert_eq!(line.value_at(6000), 1200);
    }

    #[test]
    fn test_polyline_interest_rate_point_maybe_uninit() {
        // Test initialized point
        let initialized_point = PolylineInterestRatePoint {
            utilization_rate_bps: 1000,
            borrow_rate_bps: 500,
        };
        assert!(initialized_point.maybe_uninit().is_some());

        // Test uninitialized point (borrow_rate_bps = 0)
        let uninitialized_point = PolylineInterestRatePoint {
            utilization_rate_bps: 1000,
            borrow_rate_bps: 0,
        };
        assert!(uninitialized_point.maybe_uninit().is_none());
    }

    #[test]
    fn test_polyline_try_new_valid_single_point() {
        let points = vec![PolylineInterestRatePoint {
            utilization_rate_bps: 0,
            borrow_rate_bps: 100,
        }];
        assert!(PolylineInterestRateCurve::try_new(&points).is_ok());
    }

    #[test]
    fn test_polyline_try_new_valid_multiple_points() {
        let points = vec![
            PolylineInterestRatePoint {
                utilization_rate_bps: 0,
                borrow_rate_bps: 100,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 5000,
                borrow_rate_bps: 500,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 8000,
                borrow_rate_bps: 1000,
            },
        ];
        assert!(PolylineInterestRateCurve::try_new(&points).is_ok());
    }

    #[test]
    fn test_polyline_try_new_fails_too_many_points() {
        let points = vec![
            PolylineInterestRatePoint {
                utilization_rate_bps: 0,
                borrow_rate_bps: 100,
            };
            POLYLINE_MAX_POINTS + 1
        ];

        let curve = PolylineInterestRateCurve::try_new(&points);
        assert_eq!(curve, Err(PolylineInterestRateCurveError::TooManyPoints));
    }

    #[test]
    fn test_polyline_try_new_fails_first_point_not_zero_utilization() {
        let points = vec![PolylineInterestRatePoint {
            utilization_rate_bps: 1000, // Should be 0
            borrow_rate_bps: 100,
        }];

        let curve = PolylineInterestRateCurve::try_new(&points);
        assert_eq!(
            curve,
            Err(PolylineInterestRateCurveError::FirstPointInvalid)
        );
    }

    #[test]
    fn test_polyline_try_new_fails_first_point_zero_borrow_rate() {
        let points = vec![PolylineInterestRatePoint {
            utilization_rate_bps: 0,
            borrow_rate_bps: 0, // Should be non-zero
        }];

        let curve = PolylineInterestRateCurve::try_new(&points);
        assert_eq!(
            curve,
            Err(PolylineInterestRateCurveError::FirstPointInvalid)
        );
    }

    #[test]
    fn test_polyline_try_new_fails_empty_points() {
        let points = vec![];
        let curve = PolylineInterestRateCurve::try_new(&points);
        assert_eq!(
            curve,
            Err(PolylineInterestRateCurveError::FirstPointInvalid)
        );
    }

    #[test]
    fn test_polyline_try_new_fails_descending_utilization() {
        let points = vec![
            PolylineInterestRatePoint {
                utilization_rate_bps: 0,
                borrow_rate_bps: 100,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 5000,
                borrow_rate_bps: 500,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 3000, // Lower than previous
                borrow_rate_bps: 1000,
            },
        ];

        let curve = PolylineInterestRateCurve::try_new(&points);
        assert_eq!(curve, Err(PolylineInterestRateCurveError::PointsNotInOrder));
    }

    #[test]
    fn test_polyline_try_new_fails_decreasing_borrow_rate() {
        let points = vec![
            PolylineInterestRatePoint {
                utilization_rate_bps: 0,
                borrow_rate_bps: 100,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 5000,
                borrow_rate_bps: 500,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 8000,
                borrow_rate_bps: 300, // Lower than previous
            },
        ];

        let curve = PolylineInterestRateCurve::try_new(&points);
        assert_eq!(curve, Err(PolylineInterestRateCurveError::PointsNotInOrder));
    }

    #[test]
    fn test_polyline_points_iterator() {
        let points = vec![
            PolylineInterestRatePoint {
                utilization_rate_bps: 0,
                borrow_rate_bps: 100,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 5000,
                borrow_rate_bps: 500,
            },
        ];

        let curve = PolylineInterestRateCurve::try_new(&points).unwrap();
        let collected_points: Vec<_> = curve.points().collect();

        assert_eq!(collected_points.len(), 2);
        assert_eq!(collected_points[0].utilization_rate_bps, 0);
        assert_eq!(collected_points[0].borrow_rate_bps, 100);
        assert_eq!(collected_points[1].utilization_rate_bps, 5000);
        assert_eq!(collected_points[1].borrow_rate_bps, 500);
    }

    #[test]
    fn test_polyline_borrow_rate_bps_single_point() {
        let points = vec![PolylineInterestRatePoint {
            utilization_rate_bps: 0,
            borrow_rate_bps: 100,
        }];

        let curve = PolylineInterestRateCurve::try_new(&points).unwrap();

        // Any utilization rate should return the single rate
        assert_eq!(curve.apr_borrow_rate_bps(0), 100);
        assert_eq!(curve.apr_borrow_rate_bps(5000), 100);
        assert_eq!(curve.apr_borrow_rate_bps(10000), 100);
    }

    #[test]
    fn test_polyline_borrow_rate_bps_interpolation() {
        let points = vec![
            PolylineInterestRatePoint {
                utilization_rate_bps: 0,
                borrow_rate_bps: 200,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 5000,
                borrow_rate_bps: 1000,
            },
        ];

        let curve = PolylineInterestRateCurve::try_new(&points).unwrap();

        // Test exact points
        assert_eq!(curve.apr_borrow_rate_bps(0), 200);
        assert_eq!(curve.apr_borrow_rate_bps(5000), 1000);

        // Test interpolation
        assert_eq!(curve.apr_borrow_rate_bps(2500), 600); // Midpoint
        assert_eq!(curve.apr_borrow_rate_bps(1250), 400); // Quarter point

        // Test beyond last point
        assert_eq!(curve.apr_borrow_rate_bps(10000), 1800);
    }

    #[test]
    fn test_polyline_borrow_rate_bps_multiple_segments() {
        let points = vec![
            PolylineInterestRatePoint {
                utilization_rate_bps: 0,
                borrow_rate_bps: 100,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 2000,
                borrow_rate_bps: 300,
            },
            PolylineInterestRatePoint {
                utilization_rate_bps: 8000,
                borrow_rate_bps: 1500,
            },
        ];

        let curve = PolylineInterestRateCurve::try_new(&points).unwrap();

        // Test first segment (0 to 2000)
        assert_eq!(curve.apr_borrow_rate_bps(0), 100);
        assert_eq!(curve.apr_borrow_rate_bps(1000), 200); // Midpoint of first segment
        assert_eq!(curve.apr_borrow_rate_bps(2000), 300);

        // Test second segment (2000 to 8000)
        assert_eq!(curve.apr_borrow_rate_bps(5000), 900); // Midpoint of second segment
        assert_eq!(curve.apr_borrow_rate_bps(8000), 1500);

        // Test beyond last point
        assert_eq!(curve.apr_borrow_rate_bps(10000), 1900);
    }
}
