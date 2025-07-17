use core::{cell::Cell, future::Future, pin::Pin};

use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, Mutex},
    waitqueue::AtomicWaker,
};
use libtock_console as console;
pub type Console = console::Console<super::runtime::TockSyscalls>;
pub use console::ConsoleWriter;
use libtock_platform::{
    allow_ro::AllowRoBuffer, allow_rw::AllowRwBuffer, subscribe::OneId, DefaultConfig, ErrorCode,
    Syscalls, Upcall,
};
use libtock_runtime::TockSyscalls;
use portable_atomic::AtomicBool;

pub struct ConsoleAsync;
static STORAGE: ConsoleAsyncStorage = ConsoleAsyncStorage::new();

pub struct ConsoleBufWriter<const SIZE: usize> {
    buf: [u8; SIZE],
    pos: usize,
}

impl<const SIZE: usize> ConsoleBufWriter<SIZE> {
    pub fn new() -> Self {
        Self {
            buf: [0; SIZE],
            pos: 0,
        }
    }

    pub fn into_allow_ro_buffer(self) -> ConsoleAsyncAllowRoBuffer<SIZE> {
        self.into()
    }
}

impl<const SIZE: usize> core::fmt::Write for ConsoleBufWriter<SIZE> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if s.len() + self.pos > SIZE {
            return Err(core::fmt::Error);
        }

        self.buf[self.pos..][..s.len()].copy_from_slice(s.as_bytes());
        self.pos += s.len();
        Ok(())
    }
}

type ConsoleAsyncAllowRoBuffer<const SIZE: usize> =
    AllowRoBuffer<TockSyscalls, DRIVER_NUM, { allow_ro::WRITE }, SIZE>;

type ConsoleAsyncAllowRwBuffer<const SIZE: usize> =
    AllowRwBuffer<TockSyscalls, DRIVER_NUM, { allow_rw::READ }, SIZE>;

impl<const SIZE: usize> Into<ConsoleAsyncAllowRoBuffer<SIZE>> for ConsoleBufWriter<SIZE> {
    fn into(self) -> ConsoleAsyncAllowRoBuffer<SIZE> {
        ConsoleAsyncAllowRoBuffer::<SIZE>::from_array(self.buf)
    }
}

struct ConsoleAsyncStorage {
    waker: AtomicWaker,
    busy: AtomicBool,
    result: Mutex<CriticalSectionRawMutex, Cell<Option<(u32, Result<(), ErrorCode>)>>>,
}

impl ConsoleAsyncStorage {
    const fn new() -> Self {
        Self {
            waker: AtomicWaker::new(),
            busy: AtomicBool::new(false),
            result: Mutex::new(Cell::new(None)),
        }
    }
}

impl ConsoleAsync {
    pub async fn write<const SIZE: usize>(
        s: &mut Pin<&mut ConsoleAsyncAllowRoBuffer<SIZE>>,
    ) -> Result<(), ErrorCode> {
        loop {
            match Self::try_write(s).await {
                Err(ErrorCode::Busy) => embassy_futures::yield_now().await,
                result => return result,
            }
        }
    }

    pub async fn try_write<const SIZE: usize>(
        s: &mut Pin<&mut ConsoleAsyncAllowRoBuffer<SIZE>>,
    ) -> Result<(), ErrorCode> {
        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::SeqCst)
        {
            return Err(ErrorCode::Busy);
        }

        let res = s.allow::<DefaultConfig>().and(
            TockSyscalls::command(DRIVER_NUM, command::WRITE, SIZE as u32, 0)
                .to_result::<(), ErrorCode>(),
        );
        let res = if res.is_ok() {
            Transaction.await.1
        } else {
            res
        };

        STORAGE
            .busy
            .store(false, core::sync::atomic::Ordering::SeqCst);
        res
    }

    pub async fn try_read<const SIZE: usize>(
        buf: &mut Pin<&mut ConsoleAsyncAllowRwBuffer<SIZE>>,
    ) -> (u32, Result<(), ErrorCode>) {
        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::SeqCst)
        {
            return (0u32, Err(ErrorCode::Busy));
        }

        let res = buf.allow::<DefaultConfig>().and(
            TockSyscalls::command(DRIVER_NUM, command::READ, SIZE as u32, 0)
                .to_result::<(), ErrorCode>(),
        );

        let res = if res.is_ok() {
            Transaction.await
        } else {
            (0, res)
        };

        STORAGE
            .busy
            .store(false, core::sync::atomic::Ordering::SeqCst);
        res
    }
}

struct Transaction;

impl Future for Transaction {
    type Output = (u32, Result<(), ErrorCode>);

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        STORAGE.waker.register(cx.waker());

        STORAGE.result.lock(|result| match result.take() {
            Some(res) => core::task::Poll::Ready(res),
            None => core::task::Poll::Pending,
        })
    }
}

pub struct EmbassyListener;

impl Upcall<OneId<DRIVER_NUM, { subscribe::READ }>> for EmbassyListener {
    fn upcall(&self, status: u32, bytes_received: u32, _arg2: u32) {
        let r = match status {
            0 => Ok(()),
            e_status => Err(e_status.try_into().unwrap_or(ErrorCode::Fail)),
        };

        STORAGE
            .result
            .lock(|res| res.set(Some((bytes_received, r))));
        STORAGE.waker.wake();
    }
}

impl Upcall<OneId<DRIVER_NUM, { subscribe::WRITE }>> for EmbassyListener {
    fn upcall(&self, bytes_written: u32, _arg1: u32, _arg2: u32) {
        STORAGE
            .result
            .lock(|res| res.set(Some((bytes_written, Ok(())))));
        STORAGE.waker.wake();
    }
}

// -----------------------------------------------------------------------------
// Driver number and command IDs
// -----------------------------------------------------------------------------

pub const DRIVER_NUM: u32 = 0x1;

// Command IDs
#[allow(unused)]
mod command {
    pub const EXISTS: u32 = 0;
    pub const WRITE: u32 = 1;
    pub const READ: u32 = 2;
    pub const ABORT: u32 = 3;
}

#[allow(unused)]
pub mod subscribe {
    pub const WRITE: u32 = 1;
    pub const READ: u32 = 2;
}

mod allow_ro {
    pub const WRITE: u32 = 1;
}

mod allow_rw {
    pub const READ: u32 = 1;
}
