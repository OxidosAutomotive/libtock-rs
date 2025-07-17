use crate::share::List;
use crate::Syscalls;
use core::cell::Cell;
use core::marker::PhantomData;
use core::pin::Pin;

// -----------------------------------------------------------------------------
// `AllowRo` struct
// -----------------------------------------------------------------------------

/// A `share::Handle<AllowRo>` instance allows safe code to call Tock's
/// Read-Only Allow system call, by guaranteeing the buffer will be revoked
/// before 'share ends. It is intended for use with the `share::scope` function,
/// which offers a safe interface for constructing `share::Handle<AllowRo>`
/// instances.
pub struct AllowRo<'share, S: Syscalls, const DRIVER_NUM: u32, const BUFFER_NUM: u32> {
    _syscalls: PhantomData<S>,

    // Make this struct invariant with respect to the 'share lifetime.
    //
    // If AllowRo were covariant with respect to 'share, then an
    // `AllowRo<'static, ...>` could be used to share a buffer that has a
    // shorter lifetime. The capsule would still have access to the memory after
    // the buffer is deallocated and the memory re-used (e.g. if the buffer is
    // on the stack), likely leaking data the process binary does not want to
    // share. Therefore, AllowRo cannot be covariant with respect to 'share.
    // Contravariance would not have this issue, but would still be confusing
    // and would be unexpected.
    //
    // Additionally, this makes AllowRo !Sync, which is probably desirable, as
    // Sync would allow for races between threads sharing buffers with the
    // kernel.
    _share: PhantomData<core::cell::Cell<&'share [u8]>>,
}

// We can't derive(Default) because S is not Default, and derive(Default)
// generates a Default implementation that requires S to be Default. Instead, we
// manually implement Default.
impl<'share, S: Syscalls, const DRIVER_NUM: u32, const BUFFER_NUM: u32> Default
    for AllowRo<'share, S, DRIVER_NUM, BUFFER_NUM>
{
    fn default() -> Self {
        Self {
            _syscalls: PhantomData,
            _share: PhantomData,
        }
    }
}

impl<'share, S: Syscalls, const DRIVER_NUM: u32, const BUFFER_NUM: u32> Drop
    for AllowRo<'share, S, DRIVER_NUM, BUFFER_NUM>
{
    fn drop(&mut self) {
        S::unallow_ro(DRIVER_NUM, BUFFER_NUM);
    }
}

impl<'share, S: Syscalls, const DRIVER_NUM: u32, const BUFFER_NUM: u32> List
    for AllowRo<'share, S, DRIVER_NUM, BUFFER_NUM>
{
}

pub struct AllowRoBuffer<
    S: Syscalls,
    const DRIVER_NUM: u32,
    const BUFFER_NUM: u32,
    const BUFFER_SIZE: usize,
> {
    allowed: Cell<bool>,
    pub buffer: [u8; BUFFER_SIZE],
    _syscalls: PhantomData<S>,
}

impl<S: Syscalls, const DRIVER_NUM: u32, const BUFFER_NUM: u32, const BUFFER_SIZE: usize> Default
    for AllowRoBuffer<S, DRIVER_NUM, BUFFER_NUM, BUFFER_SIZE>
{
    fn default() -> Self {
        Self {
            allowed: Cell::new(false),
            buffer: [0u8; BUFFER_SIZE],
            _syscalls: Default::default(),
        }
    }
}

impl<S: Syscalls, const DRIVER_NUM: u32, const BUFFER_NUM: u32, const BUFFER_SIZE: usize>
    AllowRoBuffer<S, DRIVER_NUM, BUFFER_NUM, BUFFER_SIZE>
{
    pub(crate) unsafe fn buffer_ptr(self: &mut core::pin::Pin<&mut Self>) -> *const u8 {
        self.buffer.as_ptr()
    }

    pub fn allow<C: Config>(self: &mut core::pin::Pin<&mut Self>) -> Result<(), crate::ErrorCode> {
        if !self.allowed.get() {
            self.allowed.set(true);
            S::allow_ro_buffer::<C, DRIVER_NUM, BUFFER_NUM, BUFFER_SIZE>(self)
        } else {
            Ok(())
        }
    }

    pub fn unallow(&mut self) {
        if self.allowed.get() {
            self.allowed.set(false);
            S::unallow_ro(DRIVER_NUM, BUFFER_NUM);
        }
    }

    pub fn from_array(buffer: [u8; BUFFER_SIZE]) -> Self {
        Self {
            allowed: Cell::new(false),
            buffer,
            _syscalls: Default::default(),
        }
    }

    pub fn get_mut_buffer(self: Pin<&mut Self>) -> &mut [u8; BUFFER_SIZE] {
        if self.allowed.get() {
            self.allowed.set(false);
            S::unallow_ro(DRIVER_NUM, BUFFER_NUM);
        }
        &mut unsafe { self.get_unchecked_mut() }.buffer
    }
}

impl<S: Syscalls, const DRIVER_NUM: u32, const BUFFER_NUM: u32, const BUFFER_SIZE: usize> Drop
    for AllowRoBuffer<S, DRIVER_NUM, BUFFER_NUM, BUFFER_SIZE>
{
    fn drop(&mut self) {
        self.unallow();
    }
}

// -----------------------------------------------------------------------------
// `Config` trait
// -----------------------------------------------------------------------------

/// `Config` configures the behavior of the Read-Only Allow system call. It
/// should generally be passed through by drivers, to allow application code to
/// configure error handling.
pub trait Config {
    /// Called if a Read-Only Allow call succeeds and returns a non-zero buffer.
    /// In some applications, this may indicate unexpected reentrance. By
    /// default, the non-zero buffer is ignored.
    fn returned_nonzero_buffer(_driver_num: u32, _buffer_num: u32) {}
}
