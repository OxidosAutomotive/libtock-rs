pub type Alarm = libtock_alarm::Alarm<super::runtime::TockSyscalls>;
pub use libtock_alarm::{AlarmListener, Convert, Hz, Milliseconds, Ticks};

use libtock_platform::subscribe::OneId;
use libtock_platform::Upcall;
use once_cell::sync::Lazy;
use portable_atomic::{AtomicBool, AtomicU32};

use core::cell::RefCell;
use core::u32;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time_queue_utils::Queue;

/// The userspace Alarm driver for multiplexing `async` delays. It is designed to work
/// with the timekeeping API exposed by the [`embassy_time`] crate, and does not provide
/// an interface of its own.
///
/// The driver must be initialized after configuring the subscribe handler exposed through
/// the [`EmbassyListener`].
pub struct AsyncAlarmDriver {
    overflow_next: AtomicBool,
    overflows: AtomicU32,
    queue: Mutex<CriticalSectionRawMutex, RefCell<Queue>>,
}

impl AsyncAlarmDriver {
    const fn new() -> AsyncAlarmDriver {
        Self {
            queue: Mutex::new(RefCell::new(Queue::new())),
            overflows: AtomicU32::new(0),
            overflow_next: AtomicBool::new(false),
        }
    }

    /// This function must be called in the "prelude" of the main function of
    /// applications using the asynchronous [`embassy_time::Timer`] interface to
    /// enable the overflow count.
    ///
    /// # Panics
    ///
    /// This function will `panic` if the `Alarm` capsule does not exist in kernel.
    fn init() {
        Alarm::exists().expect("`Alarm` capsule does not exist");
        DRIVER
            .overflow_next
            .store(true, core::sync::atomic::Ordering::Relaxed);

        let now = Alarm::get_ticks().unwrap();
        Alarm::set_absolute(Ticks(now), Ticks(u32::MAX - now)).unwrap();
    }
}

pub fn init_async_driver() {
    AsyncAlarmDriver::init();
}

impl embassy_time_driver::Driver for AsyncAlarmDriver {
    fn now(&self) -> u64 {
        let overflows = self.overflows.load(core::sync::atomic::Ordering::Relaxed) as u64;
        // SAFETY: Fails only in case the capsule does not exist
        Alarm::get_ticks().unwrap() as u64 + (overflows << 32)
    }

    fn schedule_wake(&self, at: u64, waker: &core::task::Waker) {
        critical_section::with(|cs| {
            let mut queue = self.queue.borrow(cs).borrow_mut();

            if queue.schedule_wake(at, waker) {
                let now = self.now();
                let mut next = queue.next_expiration(now);

                while next <= now {
                    next = queue.next_expiration(now);
                }

                self.set_alarm(now, next);
            }

            drop(queue);
        });
    }

    fn frequency() -> u64 {
        static FREQ: Lazy<u64> = Lazy::new(|| Alarm::get_frequency().unwrap().0 as u64);
        *FREQ
    }
}

impl AsyncAlarmDriver {
    /// Arms an alarm at the provided `timestamp`, if it will trigger before the underlying
    /// timer overflows.
    fn set_alarm(&self, now: u64, timestamp: u64) {
        let next_overflow = now | (u32::MAX as u64);
        if timestamp < next_overflow {
            let _ = Alarm::cancel();
            DRIVER
                .overflow_next
                .store(false, core::sync::atomic::Ordering::Relaxed);

            // SAFETY: set absolute command does not fail unless the Alarm capsule does not exist
            let _ =
                Alarm::set_absolute(Ticks(now as u32), Ticks(timestamp.wrapping_sub(now) as u32));
        }
    }
}

embassy_time_driver::time_driver_impl!(static DRIVER: AsyncAlarmDriver = AsyncAlarmDriver::new());

/// Structure used for registering the handler that wakes the
/// `async` tasks, called when the previously set alarm expires.
pub struct EmbassyListener;

impl Upcall<OneId<DRIVER_NUM, { subscribe::CALLBACK }>> for EmbassyListener {
    fn upcall(&self, now: u32, _deadline: u32, _arg2: u32) {
        // Checks that the current upcall signaled a timer overflow or the expiration
        // of an alarm.
        let overflows = if DRIVER
            .overflow_next
            .fetch_and(false, core::sync::atomic::Ordering::Relaxed)
        {
            DRIVER
                .overflows
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed)
                + 1
        } else {
            DRIVER.overflows.load(core::sync::atomic::Ordering::Relaxed)
        };
        let now = ((overflows as u64) << 32) + now as u64;
        let next_overflow = now | (u32::MAX as u64);

        critical_section::with(|cs| {
            // Dequeues all expired tasks and arms the timer to trigger at either the next
            // alarm deadline or the next overflow, depending on which is closer to the present
            // moment.

            let mut queue = DRIVER.queue.borrow(cs).borrow_mut();
            let mut next = queue.next_expiration(now);

            while next <= now {
                next = queue.next_expiration(now);
            }
            drop(queue);

            if next_overflow <= next {
                DRIVER
                    .overflow_next
                    .store(true, core::sync::atomic::Ordering::Relaxed);
                // SAFETY: set absolute command does not fail unless the Alarm capsule does not exist
                let _ = Alarm::set_absolute(Ticks(now as u32), Ticks((next_overflow - now) as u32));
            } else {
                // SAFETY: set absolute command does not fail unless the Alarm capsule does not exist
                let _ = Alarm::set_absolute(Ticks(now as u32), Ticks((next - now) as u32));
            }
        });
    }
}

// // -----------------------------------------------------------------------------
// // Driver number and command IDs
// // -----------------------------------------------------------------------------

pub const DRIVER_NUM: u32 = 0x0;

pub mod subscribe {
    pub const CALLBACK: u32 = 0;
}
