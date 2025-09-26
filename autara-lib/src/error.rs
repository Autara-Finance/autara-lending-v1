use std::ops::Deref;

use num_enum::{IntoPrimitive, TryFromPrimitive};

pub type LendingResult<T = ()> = Result<T, ErrorWithStack<LendingError>>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("error = {error:?}, stack = {stack:?}")]
pub struct ErrorWithStack<T> {
    pub error: T,
    pub stack: Vec<DisplayLocation>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct DisplayLocation(pub &'static std::panic::Location<'static>);

impl std::fmt::Debug for DisplayLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T> ErrorWithStack<T> {
    pub fn new(error: T, location: &'static std::panic::Location<'static>) -> Self {
        let mut context = Vec::with_capacity(4);
        context.push(DisplayLocation(location));
        ErrorWithStack {
            error,
            stack: context,
        }
    }
}

impl<T> Deref for ErrorWithStack<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.error
    }
}

impl PartialEq<LendingError> for ErrorWithStack<LendingError> {
    fn eq(&self, other: &LendingError) -> bool {
        self.error == *other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum LendingError {
    MathOverflow,
    AdditionOverflow,
    SubtractionOverflow,
    MultiplicationOverflow,
    DivisionOverflow,
    DivisionByZero,
    CastOverflow,
    MaxLtvReached,
    MaxUtilisationRateReached,
    InvalidMarketForPosition,
    PositionIsHealthy,
    MaxSupplyReached,
    InvalidLtvConfig,
    InvalidCurve,
    InvalidExpArg,
    InvalidMaxUtilisationRate,
    InvalidLiquidationLtvShouldDecrease,
    InvalidPythOracleAccount,
    InvalidOracleFeedId,
    FailedToLoadAccount,
    WithdrawalExceedsReserves,
    WithdrawalExceedsDeposited,
    RepayExceedsBorrowed,
    OracleRateTooOld,
    OracleRateRelativeConfidenceTooLow,
    NegativeOracleRate,
    LiquidationDidNotMeetRequirements,
    FeeTooHigh,
    SharesOverflow,
    InvalidNomination,
    CantModifySharePriceIfZeroShares,
    NegativeInterestRate,
    CannotSocializeDebtForHealthyPosition,
    UnsupportedMintDecimals,
}

impl LendingError {
    pub fn with_context(
        self,
        location: &'static std::panic::Location<'static>,
    ) -> ErrorWithStack<LendingError> {
        ErrorWithStack::new(self, location)
    }
}

impl<T> From<T> for ErrorWithStack<T> {
    #[track_caller]
    fn from(error: T) -> Self {
        Self::new(error, std::panic::Location::caller())
    }
}

pub trait StackTrace: Sized {
    #[track_caller]
    fn track_caller(self) -> Self;
}

impl<T> StackTrace for LendingResult<T> {
    #[inline(always)]
    fn track_caller(self) -> Self {
        let caller = std::panic::Location::caller();
        self.map_err(|mut err| {
            err.stack.push(DisplayLocation(caller));
            err
        })
    }
}

#[macro_export]
macro_rules! with_context {
    ( $error:expr) => {{
        let caller = std::panic::Location::caller();
        || $error.with_context(caller)
    }};
}

#[macro_export]
macro_rules! map_context {
    ($error:expr) => {{
        let caller = std::panic::Location::caller();
        |_| $error.with_context(caller)
    }};
}
