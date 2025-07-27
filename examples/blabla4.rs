
#![no_std]
#![no_main]

use libtock::platform::AllowRw;
use libtock::platform::DefaultConfig;
use libtock::platform::Syscalls;
use libtock::platform::share::Handle;
use libtock::runtime::TockSyscalls;

use core::cell::Cell;

pub struct Blabla<'a> {
    buffer: Option<&'a mut [u8]>,
}

impl<'a> Blabla<'a> {
    pub fn new(
        buffer: &'a mut [u8],
    ) -> Self {
        Self {
            buffer: Some(buffer),
        }
    }

    fn do_stuff<S: Syscalls>(&mut self, handle: Handle<(AllowRw<'a, S, 0x1, 0x1>,)>) {
        let buffer = self.buffer.take().unwrap();

        let (allow_rw,) = handle.split();
        S::allow_rw::<DefaultConfig, 0x1, 0x1>(allow_rw, buffer);

        /* ... */
    }
}

pub fn main() {
    let mut buffer = [0u8; 0x1000];

    libtock::platform::share::scope(|handle| {
        let mut blabla = Blabla::new(&mut buffer);

        loop {
            blabla.do_stuff::<TockSyscalls>(handle);
        }
    });
}
