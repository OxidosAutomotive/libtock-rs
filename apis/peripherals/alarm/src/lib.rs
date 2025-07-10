#![no_std]

use core::cell::Cell;
use libtock_platform::share::{self, Handle};
use libtock_platform::subscribe::{OneId, Subscribe};
use libtock_platform::{self as platform, Upcall};
use libtock_platform::{DefaultConfig, ErrorCode, Syscalls};

/// The alarm driver
///
/// # Example
/// ```ignore
/// use libtock2::Alarm;
///
/// // Wait for timeout
/// Alarm::sleep(Alarm::Milliseconds(2500));
/// ```
pub struct Alarm<S: Syscalls, C: platform::subscribe::Config = DefaultConfig>(S, C);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Hz(pub u32);

pub trait Convert {
    /// Converts a time unit by rounding up.
    fn to_ticks(self, freq: Hz) -> Ticks;
}

#[derive(Copy, Clone, Debug)]
pub struct Ticks(pub u32);

impl Convert for Ticks {
    fn to_ticks(self, _freq: Hz) -> Ticks {
        self
    }
}

#[derive(Copy, Clone)]
pub struct Milliseconds(pub u32);

impl Convert for Milliseconds {
    fn to_ticks(self, freq: Hz) -> Ticks {
        // Saturating multiplication will top out at about 1 hour at 1MHz.
        // It's large enough for an alarm, and much simpler than failing
        // or losing precision for short sleeps.

        /// u32::div_ceil is still unstable.
        fn div_ceil(a: u32, other: u32) -> u32 {
            let d = a / other;
            let m = a % other;
            if m == 0 {
                d
            } else {
                d + 1
            }
        }
        Ticks(div_ceil(self.0.saturating_mul(freq.0), 1000))
    }
}

impl<S: Syscalls, C: platform::subscribe::Config> Alarm<S, C> {
    /// Run a check against the console capsule to ensure it is present.
    #[inline(always)]
    pub fn exists() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, command::EXISTS, 0, 0).to_result()
    }

    pub fn get_frequency() -> Result<Hz, ErrorCode> {
        S::command(DRIVER_NUM, command::FREQUENCY, 0, 0)
            .to_result()
            .map(Hz)
    }

    pub fn get_ticks() -> Result<u32, ErrorCode> {
        S::command(DRIVER_NUM, command::TIME, 0, 0).to_result()
    }

    pub fn get_milliseconds() -> Result<u64, ErrorCode> {
        let ticks = Self::get_ticks()? as u64;
        let freq = (Self::get_frequency()?).0 as u64;

        Ok(ticks.saturating_div(freq / 1000))
    }

    pub fn sleep_for<T: Convert>(time: T) -> Result<(), ErrorCode> {
        let freq = Self::get_frequency()?;
        let ticks = time.to_ticks(freq);

        let called: Cell<Option<(u32, u32)>> = Cell::new(None);
        share::scope(|subscribe| {
            S::subscribe::<_, _, C, DRIVER_NUM, { subscribe::CALLBACK }>(subscribe, &called)?;

            S::command(DRIVER_NUM, command::SET_RELATIVE, ticks.0, 0)
                .to_result()
                .map(|_when: u32| ())?;

            loop {
                S::yield_wait();
                if let Some((_when, _ref)) = called.get() {
                    return Ok(());
                }
            }
        })
    }

    pub fn set_relative<T: Convert>(time: T) -> Result<u32, ErrorCode> {
        let freq = Self::get_frequency()?;
        S::command(DRIVER_NUM, command::SET_RELATIVE, time.to_ticks(freq).0, 0).to_result()
    }

    pub fn set_absolute<T: Convert, R: Convert>(reference: R, time: T) -> Result<u32, ErrorCode> {
        let freq = Self::get_frequency()?;
        S::command(
            DRIVER_NUM,
            command::SET_ABSOLUTE,
            reference.to_ticks(freq).0,
            time.to_ticks(freq).0,
        )
        .to_result()
    }

    pub fn cancel() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, command::STOP, 0, 0).to_result()
    }

    pub fn register_listener<'share, F: Fn(u32, u32)>(
        listener: &'share AlarmListener<F>,
        subscribe: Handle<Subscribe<'share, S, DRIVER_NUM, { subscribe::CALLBACK }>>,
    ) -> Result<(), ErrorCode> {
        S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::CALLBACK }>(
            subscribe, listener,
        )
    }

    pub fn unregister_listener() {
        S::unsubscribe(DRIVER_NUM, subscribe::CALLBACK);
    }
}

pub struct AlarmListener<F: Fn(u32, u32)>(pub F);

impl<F: Fn(u32, u32)> Upcall<OneId<DRIVER_NUM, 0>> for AlarmListener<F> {
    fn upcall(&self, now: u32, expiration: u32, _arg2: u32) {
        self.0(now, expiration)
    }
}

#[cfg(test)]
mod tests;

// -----------------------------------------------------------------------------
// Driver number and command IDs
// -----------------------------------------------------------------------------

const DRIVER_NUM: u32 = 0x0;

// Command IDs
#[allow(unused)]
mod command {
    pub const EXISTS: u32 = 0;
    pub const FREQUENCY: u32 = 1;
    pub const TIME: u32 = 2;
    pub const STOP: u32 = 3;

    pub const SET_RELATIVE: u32 = 5;
    pub const SET_ABSOLUTE: u32 = 6;
}

#[allow(unused)]
mod subscribe {
    pub const CALLBACK: u32 = 0;
}
