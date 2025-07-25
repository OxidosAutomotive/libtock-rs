use super::return_variant::FAILURE_2_U32;
use super::syscall_class;
use super::ErrorCode;
use super::RawSyscalls;
use super::ReturnVariant;

use core::marker::PhantomData;

#[repr(u16)]
enum Version {
    Zero = 0,
}

#[repr(transparent)]
struct Flags(u16);

impl Flags {
    const fn cleared() -> Self {
        Self(0)
    }
}

#[repr(C)]
struct StreamingProcessBufferHeader {
    version: Version,
    flags: Flags,
    write_offset: u32,
}

impl StreamingProcessBufferHeader {
    const SIZE: usize = core::mem::size_of::<Self>();

    const fn new() -> Self {
        Self {
            version: Version::Zero,
            flags: Flags::cleared(),
            write_offset: 0,
        }
    }

    const fn reset(&mut self) {
        self.flags = Flags::cleared();
        self.write_offset = 0;
    }

    const fn write_offset(&self) -> u32 {
        self.write_offset
    }
}

#[repr(C)]
pub struct StreamingProcessBuffer<const PAYLOAD_SIZE: usize> {
    header: StreamingProcessBufferHeader,
    payload: [u8; PAYLOAD_SIZE],
}

impl<const PAYLOAD_SIZE: usize> StreamingProcessBuffer<PAYLOAD_SIZE> {
    const SIZE: usize = core::mem::size_of::<Self>();

    pub const fn zeroed() -> Self {
        const { assert!(PAYLOAD_SIZE != 0) }

        Self {
            header: StreamingProcessBufferHeader::new(),
            payload: [0u8; PAYLOAD_SIZE],
        }
    }

    pub const fn payload(&self) -> &[u8; PAYLOAD_SIZE] {
        &self.payload
    }
}

#[repr(transparent)]
pub struct StreamingProcessSlice<'a>(&'a mut [u8]);

impl<'a> StreamingProcessSlice<'a> {
    /// # Safety
    ///
    /// 1. `ptr` must be word-aligned and non-null
    /// 2. `ptr` must point to StreamingProcessBuffer of size `len`
    /// 3. the object pointed by `ptr` must be valid for the lifetime 'a
    /// 4. no other pointer to the object pointed by `ptr` must exist
    unsafe fn new_unchecked(ptr: *mut u8, len: usize) -> Self {
        // SAFETY: the caller guarantees that:
        //
        // 1. `ptr` is non-null
        // 2. [`ptr`; `ptr` + len) is mutable accessible for the lifetime 'a
        let slice = unsafe { core::slice::from_raw_parts_mut(ptr, len) };

        Self(slice)
    }

    fn from_buffer<const PAYLOAD_SIZE: usize>(
        buffer: &'a mut StreamingProcessBuffer<PAYLOAD_SIZE>,
    ) -> Self {
        let ptr = core::ptr::from_mut(buffer).cast();
        let len = StreamingProcessBuffer::<{ PAYLOAD_SIZE }>::SIZE;

        // SAFETY: `ptr` and `len` have been obtained from a StreamingProcessBuffer.
        unsafe { Self::new_unchecked(ptr, len) }
    }

    fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    fn to_bytes(self) -> &'a mut [u8] {
        self.0
    }

    fn header(&self) -> &StreamingProcessBufferHeader {
        let ptr = self.as_ptr().cast();
        // SAFETY: `StreamingProcessSlice` constructor guarantees that `self` points to
        // `StreamingProcessBufferHeader`
        unsafe { &*ptr }
    }

    fn header_mut(&mut self) -> &mut StreamingProcessBufferHeader {
        let ptr = self.as_mut_ptr().cast();
        // SAFETY: `StreamingProcessSlice` constructor guarantees that `self` points to
        // `StreamingProcessBufferHeader`
        unsafe { &mut *ptr }
    }

    fn payload_length(&self) -> usize {
        // CAST: Tock does not run on 16-bit platforms, so the cast does not truncate the write
        // offset.
        self.header().write_offset() as usize
    }

    fn payload(&self) -> &[u8] {
        let ptr: *const u8 = self.as_ptr().cast();
        // SAFETY: the obtained pointer is within the same allocated object, namely a
        // `StreamingProcessBuffer`
        let payload_ptr = unsafe { ptr.byte_add(StreamingProcessBufferHeader::SIZE) };
        let payload_len = self.payload_length();

        // SAFETY: the kernel guarantees that it never writes more than PAYLOAD_SIZE bytes.
        unsafe { core::slice::from_raw_parts(payload_ptr, payload_len) }
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        let ptr: *mut u8 = self.as_mut_ptr().cast();
        // SAFETY: the obtained pointer is within the same allocated object, namely a
        // `StreamingProcessBuffer`
        let payload_ptr = unsafe { ptr.byte_add(StreamingProcessBufferHeader::SIZE) };
        let payload_len = self.payload_length();

        // SAFETY: the kernel guarantees that it never writes more than PAYLOAD_SIZE bytes.
        unsafe { core::slice::from_raw_parts_mut(payload_ptr, payload_len) }
    }
}

pub struct HandleStreamingProcessSlice<
    'a,
    const DRIVER_NUMBER: u32,
    const ALLOW_NUMBER: u32,
    S: RawSyscalls,
> {
    free_slice: Option<StreamingProcessSlice<'a>>,
    _phantom_data: PhantomData<S>,
}

impl<'a, const DRIVER_NUMBER: u32, const ALLOW_NUMBER: u32, S: RawSyscalls>
    HandleStreamingProcessSlice<'a, DRIVER_NUMBER, ALLOW_NUMBER, S>
{
    fn internal_new(free_slice: StreamingProcessSlice<'a>) -> Self {
        Self {
            free_slice: Some(free_slice),
            _phantom_data: PhantomData,
        }
    }

    fn raw_allow(
        ptr: *mut u8,
        len: usize,
    ) -> Result<Option<StreamingProcessSlice<'a>>, ErrorCode> {
        // SAFETY: syscalls4's documentation indicates it can be used to call Read-Write Allow.
        // These arguments follw TRD104.
        let [reg0, reg1, reg2, _] = unsafe {
            S::syscall4::<{ syscall_class::ALLOW_RW }>([
                DRIVER_NUMBER.into(),
                ALLOW_NUMBER.into(),
                ptr.into(),
                len.into(),
            ])
        };

        let return_variant: ReturnVariant = reg0.as_u32().into();

        if return_variant == FAILURE_2_U32 {
            // SAFETY: TRD 104 guaranteees that if r0 is FAILURE_2_U32,
            // then r1 will contain a valid error code. ErrorCode is designed to be safely
            // transmuted directly from a kernel error code.
            let error_code = unsafe { core::mem::transmute::<u32, ErrorCode>(reg1.as_u32()) };
            return Err(error_code);
        } else {
            let (ptr, len): (*mut u8, usize) = (reg1.into(), reg2.into());
            let optional_slice = if ptr == core::ptr::null_mut() || len == 0 {
                None
            } else {
                // SAFETY: when constructing a handle for a streaming process slice, the caller
                // guarantees no interleaving read-write allows.
                let slice = unsafe { StreamingProcessSlice::new_unchecked(ptr, len) };
                Some(slice)
            };

            Ok(optional_slice)
        }
    }

    fn allow(slice: StreamingProcessSlice<'a>) -> Result<Option<StreamingProcessSlice<'a>>, ErrorCode> {
        let bytes = slice.to_bytes();
        let ptr = bytes.as_mut_ptr();
        let len = bytes.len();
        Self::raw_allow(ptr, len)
    }

    fn disallow() -> Result<Option<StreamingProcessSlice<'a>>, ErrorCode> {
        Self::raw_allow(core::ptr::null_mut(), 0)
    }

    /// # Safety
    ///
    /// The caller must ensure that no read-write allow for the given driver number and allow
    /// number is issued while this handle is alive.
    pub unsafe fn new<const PAYLOAD_SIZE0: usize, const PAYLOAD_SIZE1: usize>(
        buffer0: &'a mut StreamingProcessBuffer<PAYLOAD_SIZE0>,
        buffer1: &'a mut StreamingProcessBuffer<PAYLOAD_SIZE1>,
    ) -> Result<Self, ErrorCode> {
        let slice0 = StreamingProcessSlice::from_buffer(buffer0);
        let slice1 = StreamingProcessSlice::from_buffer(buffer1);

        let _ = Self::allow(slice0)?;

        let handle_streaming_process_slice = Self::internal_new(slice1);

        Ok(handle_streaming_process_slice)
    }

    pub fn swap(&mut self) -> Result<(), ErrorCode> {
        let mut free_slice = self.free_slice.take().unwrap();
        free_slice.header_mut().reset();
        let free_slice = Self::allow(free_slice)?.unwrap();
        self.free_slice = Some(free_slice);

        Ok(())
    }

    pub fn free_payload(&self) -> &[u8] {
        self.free_slice.as_ref().map(|free_slice| free_slice.payload()).unwrap()
    }

    pub fn free_payload_mut(&mut self) -> &mut [u8] {
        self.free_slice.as_mut().map(|free_slice| free_slice.payload_mut()).unwrap()
    }
}

impl<
    'a,
    const DRIVER_NUMBER: u32,
    const ALLOW_NUMBER: u32,
    S: RawSyscalls,
> Drop for HandleStreamingProcessSlice<'a, DRIVER_NUMBER, ALLOW_NUMBER, S> {
    fn drop(&mut self) {
        Self::disallow().unwrap().unwrap();
    }
}
