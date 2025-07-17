pub type I2CMaster = libtock_i2c_master::I2CMaster<super::runtime::TockSyscalls>;

use core::{cell::Cell, future::Future, pin::Pin};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, blocking_mutex::Mutex, waitqueue::AtomicWaker,
};
use libtock_platform::{
    allow_ro::AllowRoBuffer, allow_rw::AllowRwBuffer, subscribe::OneId, DefaultConfig, ErrorCode,
    Syscalls, Upcall,
};
use libtock_runtime::TockSyscalls;
use portable_atomic::AtomicBool;

pub struct AsyncI2cMaster;
static STORAGE: AsyncI2cMasterStorage = AsyncI2cMasterStorage::new();

struct AsyncI2cMasterStorage {
    waker: AtomicWaker,
    busy: AtomicBool,
    result: Mutex<CriticalSectionRawMutex, Cell<Option<Result<(), ErrorCode>>>>,
}

impl AsyncI2cMasterStorage {
    const fn new() -> Self {
        Self {
            waker: AtomicWaker::new(),
            busy: AtomicBool::new(false),
            result: Mutex::new(Cell::new(None)),
        }
    }
}

pub type I2cAllowRwBuffer<const SIZE: usize> =
    AllowRwBuffer<TockSyscalls, DRIVER_NUM, { rw_allow::MASTER }, SIZE>;

pub type I2cAllowRoBuffer<const SIZE: usize> =
    AllowRoBuffer<TockSyscalls, DRIVER_NUM, { ro_allow::MASTER }, SIZE>;

impl AsyncI2cMaster {
    pub async fn read<const SIZE: usize>(
        &mut self,
        addr: u16,
        read: &mut Pin<&mut I2cAllowRwBuffer<SIZE>>,
    ) -> Result<(), ErrorCode> {
        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::Relaxed)
        {
            return Err(ErrorCode::Busy);
        }

        let addr = addr as u32;
        let len = SIZE as u32;

        let res = read.allow::<DefaultConfig>().and(
            TockSyscalls::command(DRIVER_NUM, i2c_master_cmd::MASTER_READ, addr, len)
                .to_result::<(), ErrorCode>(),
        );

        let res = if res.is_ok() { Transaction.await } else { res };

        STORAGE
            .busy
            .store(false, core::sync::atomic::Ordering::Relaxed);

        res
    }

    pub async fn write<const SIZE: usize>(
        &mut self,
        addr: u16,
        write: &mut Pin<&mut I2cAllowRoBuffer<SIZE>>,
    ) -> Result<(), ErrorCode> {
        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::Relaxed)
        {
            return Err(ErrorCode::Busy);
        }

        let addr = addr as u32;
        let len = SIZE as u32;

        let res = write.allow::<DefaultConfig>().and(
            TockSyscalls::command(DRIVER_NUM, i2c_master_cmd::MASTER_WRITE, addr, len)
                .to_result::<(), ErrorCode>(),
        );
        let res = if res.is_ok() { Transaction.await } else { res };

        STORAGE
            .busy
            .store(false, core::sync::atomic::Ordering::Relaxed);
        res
    }

    pub async fn write_read<const READ_SIZE: usize, const WRITE_SIZE: usize>(
        &mut self,
        addr: u16,
        write: &mut Pin<&mut I2cAllowRoBuffer<WRITE_SIZE>>,
        read: &mut Pin<&mut I2cAllowRwBuffer<READ_SIZE>>,
    ) -> Result<(), ErrorCode> {
        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::Relaxed)
        {
            return Err(ErrorCode::Busy);
        }

        let addr = addr as u32;
        let cmd_arg0 = (WRITE_SIZE as u32) << 8 | addr as u32;
        let len = READ_SIZE as u32;

        let res = read
            .allow::<DefaultConfig>()
            .and(write.allow::<DefaultConfig>())
            .and(
                TockSyscalls::command(DRIVER_NUM, i2c_master_cmd::MASTER_WRITE_READ, cmd_arg0, len)
                    .to_result::<(), ErrorCode>(),
            );
        let res = if res.is_ok() { Transaction.await } else { res };

        STORAGE
            .busy
            .store(false, core::sync::atomic::Ordering::Relaxed);
        res
    }

    pub async fn write_read_in_place<const SIZE: usize>(
        &mut self,
        addr: u16,
        w_len: u16,
        r_len: u16,
        buf: &mut Pin<&mut I2cAllowRwBuffer<SIZE>>,
    ) -> Result<(), ErrorCode> {
        if (r_len as usize) > SIZE || (w_len as usize) > SIZE {
            return Err(ErrorCode::NoMem);
        }

        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::Relaxed)
        {
            return Err(ErrorCode::Busy);
        }

        let addr = addr as u32;
        let cmd_arg0 = (w_len as u32) << 8 | addr as u32;
        let len = r_len as u32;

        let res = buf.allow::<DefaultConfig>().and(
            TockSyscalls::command(
                DRIVER_NUM,
                i2c_master_cmd::MASTER_WRITE_READ_IN_PLACE,
                cmd_arg0,
                len,
            )
            .to_result::<(), ErrorCode>(),
        );
        let res = if res.is_ok() { Transaction.await } else { res };

        STORAGE
            .busy
            .store(false, core::sync::atomic::Ordering::Relaxed);
        res
    }
}

/// Structure used for registering the handler that wakes the
/// `async` tasks, called when an I2C controller transfer is completed.
pub struct EmbassyListener;

impl Upcall<OneId<DRIVER_NUM, { subscribe::MASTER_READ_WRITE }>> for EmbassyListener {
    fn upcall(&self, arg0: u32, _arg1: u32, _arg2: u32) {
        let status = match ErrorCode::try_from(arg0) {
            Ok(err) => Err(err),
            _ => Ok(()),
        };

        STORAGE.result.lock(|res| res.set(Some(status)));
        STORAGE.waker.wake();
    }
}

struct Transaction;

impl Future for Transaction {
    type Output = Result<(), ErrorCode>;

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

// -----------------------------------------------------------------------------
// Driver number and command IDs
// -----------------------------------------------------------------------------
pub const DRIVER_NUM: u32 = 0x20003;

#[allow(unused)]
pub mod subscribe {
    pub const MASTER_READ: u32 = 0;
    pub const MASTER_WRITE: u32 = 0;
    pub const MASTER_READ_WRITE: u32 = 0;
}

/// Ids for read-write allow buffers
#[allow(unused)]
mod rw_allow {
    pub const MASTER: u32 = 1;
}

/// Ids for read-write allow buffers
#[allow(unused)]
mod ro_allow {
    pub const MASTER: u32 = 0;
}

#[allow(unused)]
mod i2c_master_cmd {
    pub const EXISTS: u32 = 0;
    pub const MASTER_WRITE: u32 = 1;
    pub const MASTER_READ: u32 = 2;
    pub const MASTER_WRITE_READ_IN_PLACE: u32 = 3;
    pub const MASTER_WRITE_READ: u32 = 4;
}
