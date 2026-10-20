use core::cell::Cell;
use libtock_platform::{
    share, subscribe::OneId, AllowRo, AllowRw, DefaultConfig, ErrorCode, Subscribe, Syscalls,
    Upcall,
};

pub struct Ecdsa<S: Syscalls>(S);

impl<S: Syscalls> Ecdsa<S> {
    /// Check if the Ecdsa kernel driver exists
    pub fn exists() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, cmd::EXISTS, 0, 0).to_result()
    }

    /// Share the hash to be signed with the kernel
    pub fn allow_input_buffer<'share>(
        buf: &'share [u8],
        allow_ro: share::Handle<AllowRo<'share, S, DRIVER_NUM, { ro_allow::INPUT_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_ro::<DefaultConfig, DRIVER_NUM, { ro_allow::INPUT_BUF }>(allow_ro, buf)
    }

    pub fn unallow_input_buffer() {
        S::unallow_ro(DRIVER_NUM, ro_allow::INPUT_BUF)
    }

    /// Share the buffer the kernel writes the signature into
    pub fn allow_output_buffer<'share>(
        buf: &'share mut [u8],
        allow_rw: share::Handle<AllowRw<'share, S, DRIVER_NUM, { rw_allow::OUTPUT_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_rw::<DefaultConfig, DRIVER_NUM, { rw_allow::OUTPUT_BUF }>(allow_rw, buf)
    }

    pub fn unallow_output_buffer() {
        S::unallow_rw(DRIVER_NUM, rw_allow::OUTPUT_BUF)
    }

    /// Share the private key (32 bytes, big-endian) to load into the signer
    pub fn allow_private_key_buffer<'share>(
        buf: &'share [u8],
        allow_ro: share::Handle<AllowRo<'share, S, DRIVER_NUM, { ro_allow::PRIVATE_KEY_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_ro::<DefaultConfig, DRIVER_NUM, { ro_allow::PRIVATE_KEY_BUF }>(allow_ro, buf)
    }

    pub fn unallow_private_key_buffer() {
        S::unallow_ro(DRIVER_NUM, ro_allow::PRIVATE_KEY_BUF)
    }

    /// Share the buffer the kernel writes the public key (64 bytes, X || Y) into
    pub fn allow_public_key_buffer<'share>(
        buf: &'share mut [u8],
        allow_rw: share::Handle<AllowRw<'share, S, DRIVER_NUM, { rw_allow::PUBLIC_KEY_BUF }>>,
    ) -> Result<(), ErrorCode> {
        S::allow_rw::<DefaultConfig, DRIVER_NUM, { rw_allow::PUBLIC_KEY_BUF }>(allow_rw, buf)
    }

    pub fn unallow_public_key_buffer() {
        S::unallow_rw(DRIVER_NUM, rw_allow::PUBLIC_KEY_BUF)
    }

    /// Register an Ecdsa listener to be called when an upcall is serviced
    /// Must be used in conjunction with the `share::scope` function
    pub fn register_listener<'share, F: Fn(u32)>(
        listener: &'share EcdsaListener<F>,
        subscribe: share::Handle<Subscribe<'share, S, DRIVER_NUM, { subscribe::DONE }>>,
    ) -> Result<(), ErrorCode> {
        S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::DONE }>(subscribe, listener)
    }

    pub fn unregister_listener() {
        S::unsubscribe(DRIVER_NUM, subscribe::DONE)
    }

    /// Users must first share buffer slices with the kernel and register an Ecdsa listener
    pub fn compute_async() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, cmd::COMPUTE, 0, 0).to_result()
    }

    /// Sign the hash in `input_buf` and write the signature into `output_buf`
    pub fn compute_sync(input_buf: &[u8], output_buf: &mut [u8]) -> Result<(), ErrorCode> {
        let status: Cell<Option<Result<(), ErrorCode>>> = Cell::new(None);
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
            Self::allow_output_buffer(output_buf, allow_rw)?;

            // Subscribe for an upcall with the kernel
            S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::DONE }>(
                subscribe, &status,
            )?;

            // Send the command to the kernel driver to fill the allowed_readwrite buffer
            S::command(DRIVER_NUM, cmd::COMPUTE, 0, 0).to_result::<(), ErrorCode>()?;

            Self::wait_for_done(&status)
        })
    }

    /// Users must first share the private key buffer with the kernel and register an Ecdsa listener
    pub fn set_private_key_async() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, cmd::SET_PRIVATE_KEY, 0, 0).to_result()
    }

    /// Load `private_key` (32 bytes, big-endian, `0 < d < n`) into the signer.
    ///
    /// Every following signature is made with this key. The key cannot be
    /// read back.
    pub fn set_private_key_sync(private_key: &[u8]) -> Result<(), ErrorCode> {
        let status: Cell<Option<Result<(), ErrorCode>>> = Cell::new(None);
        share::scope::<
            (
                AllowRo<S, DRIVER_NUM, { ro_allow::PRIVATE_KEY_BUF }>,
                Subscribe<S, DRIVER_NUM, { subscribe::DONE }>,
            ),
            _,
            _,
        >(|handle| {
            let (allow_ro, subscribe) = handle.split();

            Self::allow_private_key_buffer(private_key, allow_ro)?;

            S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::DONE }>(
                subscribe, &status,
            )?;

            S::command(DRIVER_NUM, cmd::SET_PRIVATE_KEY, 0, 0).to_result::<(), ErrorCode>()?;

            Self::wait_for_done(&status)
        })
    }

    /// Users must first share the public key buffer with the kernel and register an Ecdsa listener
    pub fn get_public_key_async() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, cmd::GET_PUBLIC_KEY, 0, 0).to_result()
    }

    /// Write the public key `Q = d·G` of the loaded private key into
    /// `public_key` (64 bytes, uncompressed `X || Y`, big-endian).
    pub fn get_public_key_sync(public_key: &mut [u8]) -> Result<(), ErrorCode> {
        let status: Cell<Option<Result<(), ErrorCode>>> = Cell::new(None);
        share::scope::<
            (
                AllowRw<S, DRIVER_NUM, { rw_allow::PUBLIC_KEY_BUF }>,
                Subscribe<S, DRIVER_NUM, { subscribe::DONE }>,
            ),
            _,
            _,
        >(|handle| {
            let (allow_rw, subscribe) = handle.split();

            Self::allow_public_key_buffer(public_key, allow_rw)?;

            S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::DONE }>(
                subscribe, &status,
            )?;

            S::command(DRIVER_NUM, cmd::GET_PUBLIC_KEY, 0, 0).to_result::<(), ErrorCode>()?;

            Self::wait_for_done(&status)
        })
    }

    /// Wait for a callback to happen, then return the status it reported
    fn wait_for_done(status: &Cell<Option<Result<(), ErrorCode>>>) -> Result<(), ErrorCode> {
        loop {
            if let Some(result) = status.get() {
                return result;
            }
            S::yield_wait();
        }
    }
}

/// The provided listener to be called with the status of the finished
/// operation. All operations share the same upcall.
pub struct EcdsaListener<F: Fn(u32)>(pub F);

impl<F: Fn(u32)> Upcall<OneId<DRIVER_NUM, 0>> for EcdsaListener<F> {
    fn upcall(&self, arg0: u32, _: u32, _: u32) {
        (self.0)(arg0)
    }
}

// -------------
// DRIVER NUMBER
// -------------
const DRIVER_NUM: u32 = 0x40008;

// ---------------
// COMMAND NUMBERS
// ---------------
mod cmd {
    pub const EXISTS: u32 = 0;
    pub const COMPUTE: u32 = 1;
    pub const SET_PRIVATE_KEY: u32 = 2;
    pub const GET_PUBLIC_KEY: u32 = 3;
}
// ---------------
// READ-ONLY BUFFER NUMBERS
// ---------------
mod ro_allow {
    /// Hash to be signed
    pub const INPUT_BUF: u32 = 0;
    /// Private key to load into the signer
    pub const PRIVATE_KEY_BUF: u32 = 1;
}

// ---------------
// READ-WRITE BUFFER NUMBERS
// ---------------
mod rw_allow {
    /// Signature output
    pub const OUTPUT_BUF: u32 = 0;
    /// Public key output
    pub const PUBLIC_KEY_BUF: u32 = 1;
}

// ---------------
// UPCALL NUMBERS
// ---------------
mod subscribe {
    pub const DONE: u32 = 0;
}
