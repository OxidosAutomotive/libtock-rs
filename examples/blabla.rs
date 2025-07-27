#![no_std]
#![no_main]

use libtock::platform::AllowRw;
use libtock::platform::DefaultConfig;
use libtock::platform::Syscalls;
use libtock::platform::share::Handle;
use libtock::runtime::TockSyscalls;

pub struct Blabla<'a, S: Syscalls> {
    handle: Handle<'a, AllowRw<'a, S, 0x1, 0x1>>,
    buffer: Option<&'a mut [u8]>,
}

impl<'a, S: Syscalls> Blabla<'a, S> {
    pub fn new(
        handle: Handle<'a, AllowRw<'a, S, 0x1, 0x1>>,
        buffer: &'a mut [u8],
    ) -> Self {
        Self {
            handle,
            buffer: Some(buffer),
        }
    }

    fn do_stuff(&mut self) {
        let buffer = self.buffer.take().unwrap();
        S::allow_rw::<DefaultConfig, 0x1, 0x1>(self.handle, buffer);
        /* ... */
    }
}

pub fn main() {
    let mut buffer = [0u8; 0x1000];

    libtock::platform::share::scope::<
        (AllowRw<'_, TockSyscalls, 0x1, 0x1>,),
        _,
        _,
    >(|handle| {
        let (allow_rw,) = handle.split();
        let blabla = Blabla::new(allow_rw, &mut buffer);
    });
}
