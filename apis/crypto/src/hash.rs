use core::{cell::Cell, marker::PhantomData};
use libtock_platform::{
    share, subscribe::OneId, AllowRo, AllowRw, DefaultConfig, ErrorCode, Subscribe, Syscalls,
    Upcall,
};

use crate::DigestAlgorithm;

pub struct Hash<S: Syscalls, A: DigestAlgorithm>(PhantomData<(S, A)>);

impl<S: Syscalls, A: DigestAlgorithm> Hash<S, A> {
    /// Check if the Hash kernel driver exists
    pub fn exists() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, cmd::EXISTS, 0, 0).to_result()
    }

    pub fn allow_input_buffer<'share>(
        buf: &'share [u8],
        allow_ro: share::Handle<AllowRo<'share, S, DRIVER_NUM, { ro_allow::INPUT_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_ro::<DefaultConfig, DRIVER_NUM, { ro_allow::INPUT_BUF }>(allow_ro, buf)
    }

    pub fn unallow_input_buffer() {
        S::unallow_ro(DRIVER_NUM, ro_allow::INPUT_BUF)
    }

    pub fn allow_output_buffer<'share>(
        buf: &'share mut A::Output,
        allow_rw: share::Handle<AllowRw<'share, S, DRIVER_NUM, { rw_allow::OUTPUT_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_rw::<DefaultConfig, DRIVER_NUM, { rw_allow::OUTPUT_BUF }>(allow_rw, buf.as_mut())
    }

    pub fn unallow_output_buffer() {
        S::unallow_rw(DRIVER_NUM, rw_allow::OUTPUT_BUF)
    }

    /// Register an Rng listener to be called when an upcall is serviced
    /// Must be used in conjunction with the `share::scope` function
    pub fn register_listener<'share, F: Fn(u32)>(
        listener: &'share HashListener<F>,
        subscribe: share::Handle<Subscribe<'share, S, DRIVER_NUM, { subscribe::DONE }>>,
    ) -> Result<(), ErrorCode> {
        S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::DONE }>(subscribe, listener)
    }

    pub fn unregister_listener() {
        S::unsubscribe(DRIVER_NUM, subscribe::DONE)
    }

    /// Users must first share buffer slices with the kernel and register a Hash listener
    pub fn compute_async() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, cmd::COMPUTE, A::COMMAND_NUM, 0).to_result()
    }

    pub fn compute_sync(input_buf: &[u8]) -> Result<A::Output, ErrorCode> {
        let status: Cell<Option<(u32,)>> = Cell::new(None);
        let mut output_buf = A::ZEROED;
        share::scope::<
            (
                AllowRo<S, DRIVER_NUM, { ro_allow::INPUT_BUF }>,
                AllowRw<S, DRIVER_NUM, { rw_allow::OUTPUT_BUF }>,
                Subscribe<S, DRIVER_NUM, { subscribe::DONE }>,
            ),
            _,
            _,
        >(|handle| {
            let (allow_ro, allow_rw, subscribe) = handle.split();
            Self::allow_input_buffer(input_buf, allow_ro)?;
            Self::allow_output_buffer(&mut output_buf, allow_rw)?;

            // Subscribe for an upcall with the kernel
            S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::DONE }>(
                subscribe, &status,
            )?;

            // Send the command to the kernel driver to fill the allowed_readwrite buffer
            S::command(DRIVER_NUM, cmd::COMPUTE, A::COMMAND_NUM, 0).to_result::<(), ErrorCode>()?;

            // Wait for a callback to happen
            while status.get().is_none() {
                S::yield_wait();
            }

            match status.get() {
                Some((0,)) => Ok(()),
                _ => Err(ErrorCode::Fail),
            }
        })?;

        Ok(output_buf)
    }
}

/// The provided listener to be called.
pub struct HashListener<F: Fn(u32)>(pub F);

impl<F: Fn(u32)> Upcall<OneId<DRIVER_NUM, 0>> for HashListener<F> {
    fn upcall(&self, arg0: u32, _: u32, _: u32) {
        (self.0)(arg0)
    }
}

// -------------
// DRIVER NUMBER
// -------------
const DRIVER_NUM: u32 = 0x40005;

// ---------------
// COMMAND NUMBERS
// ---------------
mod cmd {
    pub const EXISTS: u32 = 0;
    pub const COMPUTE: u32 = 1;
}
// ---------------
// READ-ONLY BUFFER NUMBERS
// ---------------
mod ro_allow {
    pub const INPUT_BUF: u32 = 0;
}

// ---------------
// READ-WRITE BUFFER NUMBERS
// ---------------
mod rw_allow {
    pub const OUTPUT_BUF: u32 = 0;
}

// ---------------
// UPCALL NUMBERS
// ---------------
mod subscribe {
    pub const DONE: u32 = 0;
}
