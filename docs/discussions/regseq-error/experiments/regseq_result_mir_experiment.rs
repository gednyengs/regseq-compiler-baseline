#![feature(try_trait_v2)]

use std::convert::Infallible;
use std::ops::{ControlFlow, FromResidual, Try};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegseqError {
    Timeout,
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegseqResult<T> {
    Ok(T),
    Err(RegseqError),
}

impl<T> Try for RegseqResult<T> {
    type Output = T;
    type Residual = RegseqResult<Infallible>;

    fn from_output(output: Self::Output) -> Self {
        RegseqResult::Ok(output)
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            RegseqResult::Ok(value) => ControlFlow::Continue(value),
            RegseqResult::Err(error) => ControlFlow::Break(RegseqResult::Err(error)),
        }
    }
}

impl<T> FromResidual<RegseqResult<Infallible>> for RegseqResult<T> {
    fn from_residual(residual: RegseqResult<Infallible>) -> Self {
        match residual {
            RegseqResult::Ok(_) => unreachable!(),
            RegseqResult::Err(error) => RegseqResult::Err(error),
        }
    }
}

pub struct Clock;

impl Clock {
    pub fn enable(&self) -> RegseqResult<()> {
        RegseqResult::Ok(())
    }

    pub fn enable_mapped(&self) -> RegseqResult<()> {
        let status = self.enable_native();
        if status != 0 {
            return RegseqResult::Err(RegseqError::Busy);
        }
        RegseqResult::Ok(())
    }

    pub fn enable_native(&self) -> i32 {
        0
    }
}

pub struct Driver {
    pub clock: Clock,
}

pub fn explicit_authored_error() -> RegseqResult<()> {
    RegseqResult::Err(RegseqError::Timeout)
}

pub fn wrapper_mediated_error(driver: &Driver) -> RegseqResult<()> {
    driver.clock.enable()?;
    RegseqResult::Ok(())
}

pub fn wrapper_mediated_mapped_native(driver: &Driver) -> RegseqResult<()> {
    driver.clock.enable_mapped()?;
    RegseqResult::Ok(())
}

pub fn native_status_passthrough(driver: &Driver) -> i32 {
    let status = driver.clock.enable_native();
    if status != 0 {
        return status;
    }
    0
}
