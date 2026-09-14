use core::cell::Cell;
use libtock_platform::{
    share, subscribe::OneId, AllowRo, AllowRw, DefaultConfig, ErrorCode, Subscribe, Syscalls,
    Upcall,
};

use crate::HashAlgorithm;

pub struct Hkdf<S: Syscalls>(S);

impl<S: Syscalls> Hkdf<S> {
    /// Check if the Hash kernel driver exists
    pub fn exists() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, cmd::EXISTS, 0, 0).to_result()
    }

    pub fn allow_ikm_buffer<'share>(
        buf: &'share [u8],
        allow_ro: share::Handle<AllowRo<'share, S, DRIVER_NUM, { ro_allow::IKM_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_ro::<DefaultConfig, DRIVER_NUM, { ro_allow::IKM_BUF }>(allow_ro, buf)
    }

    pub fn unallow_ikm_buffer() {
        S::unallow_ro(DRIVER_NUM, ro_allow::IKM_BUF)
    }

    pub fn allow_salt_buffer<'share>(
        buf: &'share [u8],
        allow_ro: share::Handle<AllowRo<'share, S, DRIVER_NUM, { ro_allow::SALT_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_ro::<DefaultConfig, DRIVER_NUM, { ro_allow::SALT_BUF }>(allow_ro, buf)
    }

    pub fn unallow_salt_buffer() {
        S::unallow_ro(DRIVER_NUM, ro_allow::IKM_BUF)
    }

    pub fn allow_info_buffer<'share>(
        buf: &'share [u8],
        allow_ro: share::Handle<AllowRo<'share, S, DRIVER_NUM, { ro_allow::INFO_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_ro::<DefaultConfig, DRIVER_NUM, { ro_allow::INFO_BUF }>(allow_ro, buf)
    }

    pub fn unallow_info_buffer() {
        S::unallow_ro(DRIVER_NUM, ro_allow::IKM_BUF)
    }

    pub fn allow_prk_buffer<'share>(
        buf: &'share mut [u8],
        allow_rw: share::Handle<AllowRw<'share, S, DRIVER_NUM, { rw_allow::PRK_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_rw::<DefaultConfig, DRIVER_NUM, { rw_allow::PRK_BUF }>(allow_rw, buf)
    }

    pub fn unallow_prk_buffer() {
        S::unallow_rw(DRIVER_NUM, rw_allow::PRK_BUF)
    }

    pub fn allow_okm_buffer<'share>(
        buf: &'share mut [u8],
        allow_rw: share::Handle<AllowRw<'share, S, DRIVER_NUM, { rw_allow::OKM_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_rw::<DefaultConfig, DRIVER_NUM, { rw_allow::OKM_BUF }>(allow_rw, buf)
    }

    pub fn unallow_okm_buffer() {
        S::unallow_rw(DRIVER_NUM, rw_allow::OKM_BUF)
    }

    /// Register an Rng listener to be called when an upcall is serviced
    /// Must be used in conjunction with the `share::scope` function
    pub fn register_listener<'share, F: Fn(u32)>(
        listener: &'share HkdfListener<F>,
        subscribe: share::Handle<Subscribe<'share, S, DRIVER_NUM, { subscribe::DONE }>>,
    ) -> Result<(), ErrorCode> {
        S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::DONE }>(subscribe, listener)
    }

    pub fn unregister_listener() {
        S::unsubscribe(DRIVER_NUM, subscribe::DONE)
    }

    /// Users must first share buffer slices with the kernel and register a Hash listener
    pub fn compute_async(algo: HashAlgorithm) -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, cmd::COMPUTE, algo as u32, 0).to_result()
    }

    pub fn compute_sync(
        algo: HashAlgorithm,
        ikm_buf: &[u8],
        prk_buf: &mut [u8],
        okm_buf: &mut [u8],
        salt_buf: Option<&[u8]>,
        info_buf: Option<&[u8]>,
    ) -> Result<(), ErrorCode> {
        let called = Cell::new(false);
        share::scope::<
            (
                AllowRo<S, DRIVER_NUM, { ro_allow::IKM_BUF }>,
                AllowRo<S, DRIVER_NUM, { ro_allow::SALT_BUF }>,
                AllowRo<S, DRIVER_NUM, { ro_allow::INFO_BUF }>,
                AllowRw<S, DRIVER_NUM, { rw_allow::PRK_BUF }>,
                AllowRw<S, DRIVER_NUM, { rw_allow::OKM_BUF }>,
                Subscribe<S, DRIVER_NUM, { subscribe::DONE }>,
            ),
            _,
            _,
        >(|handle| {
            let (allow_ro_ikm, allow_ro_salt, allow_ro_info, allow_rw_prk, allow_rw_okm, subscribe) =
                handle.split();

            Self::allow_ikm_buffer(ikm_buf, allow_ro_ikm)?;
            Self::allow_prk_buffer(prk_buf, allow_rw_prk)?;
            Self::allow_okm_buffer(okm_buf, allow_rw_okm)?;

            match salt_buf {
                Some(buf) => Self::allow_salt_buffer(buf, allow_ro_salt)?,
                None => Self::unallow_salt_buffer(),
            };

            match info_buf {
                Some(buf) => Self::allow_info_buffer(buf, allow_ro_info)?,
                None => Self::unallow_info_buffer(),
            };

            // Subscribe for an upcall with the kernel
            S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::DONE }>(
                subscribe, &called,
            )?;

            // Send the command to the kernel driver to fill the allowed_readwrite buffer
            S::command(DRIVER_NUM, cmd::COMPUTE, algo as u32, 0).to_result::<(), ErrorCode>()?;

            // Wait for a callback to happen
            while !called.get() {
                S::yield_wait();
            }

            Ok(())
        })
    }
}

/// The provided listener to be called.
pub struct HkdfListener<F: Fn(u32)>(pub F);

impl<F: Fn(u32)> Upcall<OneId<DRIVER_NUM, 0>> for HkdfListener<F> {
    fn upcall(&self, arg0: u32, _: u32, _: u32) {
        (self.0)(arg0)
    }
}

// -------------
// DRIVER NUMBER
// -------------
const DRIVER_NUM: u32 = 0x40007;

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
    pub const SALT_BUF: u32 = 0;
    /// Input Keying Material
    pub const IKM_BUF: u32 = 1;
    /// Info
    pub const INFO_BUF: u32 = 2;
}

// ---------------
// READ-WRITE BUFFER NUMBERS
// ---------------
mod rw_allow {
    pub const PRK_BUF: u32 = 0;
    pub const OKM_BUF: u32 = 1;
}

// ---------------
// UPCALL NUMBERS
// ---------------
mod subscribe {
    pub const DONE: u32 = 0;
}
