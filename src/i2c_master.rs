pub type I2CMaster = libtock_i2c_master::I2CMaster<super::runtime::TockSyscalls>;

use core::{cell::Cell, future::Future};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, blocking_mutex::Mutex, waitqueue::AtomicWaker,
};
use embedded_hal::i2c::Operation;
use libtock_platform::{
    share, subscribe::OneId, AllowRo, AllowRw, DefaultConfig, ErrorCode, Syscalls, Upcall,
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

impl embedded_hal_async::i2c::ErrorType for AsyncI2cMaster {
    type Error = ErrorCode;
}

impl<A: embedded_hal_async::i2c::AddressMode + Into<u16> + 'static> embedded_hal_async::i2c::I2c<A>
    for AsyncI2cMaster
{
    async fn transaction(
        &mut self,
        address: A,
        operations: &mut [embedded_hal::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        let addr: u16 = address.into();

        match operations {
            [Operation::Read(read)] => self.read(addr, read).await,
            [Operation::Write(write)] => self.write(addr, write).await,
            [Operation::Write(write), Operation::Read(read)] => {
                self.write_read(addr, write, read).await
            }
            _ => Err(ErrorCode::NoSupport),
        }
    }
}

impl AsyncI2cMaster {
    async fn read(&mut self, addr: u16, read: &mut [u8]) -> Result<(), ErrorCode> {
        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::Relaxed)
        {
            return Err(ErrorCode::Busy);
        }

        let addr = addr as u32;
        let len = read.len() as u32;

        let res =
            share::async_scope::<AllowRw<TockSyscalls, DRIVER_NUM, { rw_allow::MASTER }>, _, _>(
                async |handle| {
                    TockSyscalls::allow_rw::<DefaultConfig, DRIVER_NUM, { rw_allow::MASTER }>(
                        handle, read,
                    )?;

                    TockSyscalls::command(DRIVER_NUM, i2c_master_cmd::MASTER_READ, addr, len)
                        .to_result::<(), ErrorCode>()?;

                    Transaction.await
                },
            )
            .await;

        STORAGE
            .busy
            .store(false, core::sync::atomic::Ordering::Relaxed);
        res
    }

    async fn write(&mut self, addr: u16, write: &[u8]) -> Result<(), ErrorCode> {
        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::Relaxed)
        {
            return Err(ErrorCode::Busy);
        }

        let addr = addr as u32;
        let len = write.len() as u32;

        let res =
            share::async_scope::<AllowRo<TockSyscalls, DRIVER_NUM, { ro_allow::MASTER }>, _, _>(
                async |handle| {
                    TockSyscalls::allow_ro::<DefaultConfig, DRIVER_NUM, { ro_allow::MASTER }>(
                        handle, write,
                    )?;

                    TockSyscalls::command(DRIVER_NUM, i2c_master_cmd::MASTER_WRITE, addr, len)
                        .to_result::<(), ErrorCode>()?;

                    Transaction.await
                },
            )
            .await;

        STORAGE
            .busy
            .store(false, core::sync::atomic::Ordering::Relaxed);
        res
    }

    async fn write_read(
        &mut self,
        addr: u16,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), ErrorCode> {
        if STORAGE
            .busy
            .fetch_or(true, core::sync::atomic::Ordering::Relaxed)
        {
            return Err(ErrorCode::Busy);
        }

        let addr = addr as u32;
        let cmd_arg0 = (write.len() as u32) << 8 | addr as u32;
        let len = read.len() as u32;

        let res = share::async_scope::<
            (
                AllowRo<TockSyscalls, DRIVER_NUM, { ro_allow::MASTER }>,
                AllowRw<TockSyscalls, DRIVER_NUM, { rw_allow::MASTER }>,
            ),
            _,
            _,
        >(async |handle| {
            let (allow_ro, allow_rw) = handle.split();

            TockSyscalls::allow_ro::<DefaultConfig, DRIVER_NUM, { ro_allow::MASTER }>(
                allow_ro, write,
            )?;
            TockSyscalls::allow_rw::<DefaultConfig, DRIVER_NUM, { rw_allow::MASTER }>(
                allow_rw, read,
            )?;

            TockSyscalls::command(DRIVER_NUM, i2c_master_cmd::MASTER_WRITE_READ, cmd_arg0, len)
                .to_result::<(), ErrorCode>()?;

            Transaction.await
        })
        .await;

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
