use libtock_gpio;

pub type Gpio = libtock_gpio::Gpio<super::runtime::TockSyscalls>;
pub use libtock_gpio::{
    asynchronous::{EmbassyListener, InputFuture},
    Error, GpioInterruptListener, GpioState, InputPin, OutputPin, PinInterruptEdge, Pull, PullDown,
    PullNone, PullUp,
};

// -----------------------------------------------------------------------------
// Driver number and command IDs
// -----------------------------------------------------------------------------

pub const DRIVER_NUM: u32 = 0x4;

pub mod subscribe {
    pub const CALLBACK: u32 = 0x0;
}
