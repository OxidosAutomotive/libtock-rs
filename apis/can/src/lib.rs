#![no_std]

use core::cell::Cell;
use core::mem::size_of;

use libtock_platform::{
    share::scope, share::Handle, AllowRo, AllowRw, DefaultConfig, ErrorCode, Subscribe, Syscalls,
};

pub struct Can<S: Syscalls>(S);

// #[derive(Debug, Copy, Clone, PartialEq)]
// pub enum Mode {
//     Ok,
//     Warning,
//     Passive,
//     BusOff,
// }

// /// Defines the possible states of the peripheral
// #[derive(Debug, Copy, Clone, PartialEq)]
// pub enum State {
//     /// The peripheral is enabled and functions normally
//     Running(Mode),

//     /// The peripheral is disabled
//     Disabled,
// }

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum State {
    Uninit = 0,
    Running = 1,
    Sleep = 2,
    Stopped = 3,
    BusOff = 4,
}

// fn state_from_tuple(value: (u32, u32)) -> Result<State, ErrorCode> {
//     match value {
//         (0, _) => Ok(State::Disabled),
//         (1, mode) => Ok(State::Running(match mode {
//             0 => Mode::Ok,
//             1 => Mode::Passive,
//             2 => Mode::Warning,
//             3 => Mode::BusOff,
//             _ => return Err(ErrorCode::Invalid),
//         })),
//         _ => Err(ErrorCode::Invalid),
//     }
// }

impl From<u32> for State {
    fn from(value: u32) -> Self {
        match value {
            0 => State::Uninit,
            1 => State::Running,
            2 => State::Sleep,
            3 => State::Stopped,
            _ => State::BusOff,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Id {
    Standard(u16),
    Extended(u32),
}

impl From<u32> for Id {
    fn from(value: u32) -> Id {
        if value >> 30 == 1 {
            Id::Extended(value & 0x1FFF_FFFF)
        } else {
            Id::Standard(value as u16)
        }
    }
}

impl From<Id> for u32 {
    fn from(id: Id) -> u32 {
        match id {
            // TODO: FIX THIS
            Id::Standard(id) => id as u32,
            Id::Extended(id) => (1 << 30) | (id & 0x1FFF_FFFF),
        }
    }
}

impl<S: Syscalls> Can<S> {
    pub fn driver_exists() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, EXISTS, 0, 0).to_result()
    }

    pub fn set_baudrate(baudrate: u32) -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, SET_BIT_RATE, baudrate, 0).to_result()
    }

    pub fn enable() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, ENABLE, 0, 0).to_result()
    }

    pub fn disable() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, DISABLE, 0, 0).to_result()
    }

    pub fn set_operation_mode(mode: OperationMode) -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, SET_MODE, mode as u32, 0).to_result()
    }

    fn send_message_async<'share>(
        allow_handle: Handle<AllowRo<'share, S, DRIVER_NUM, { allow_ro::MESSAGE }>>,
        frame: &'share Frame,
    ) -> Result<(), ErrorCode> {
        S::allow_ro::<DefaultConfig, DRIVER_NUM, { allow_ro::MESSAGE }>(
            allow_handle,
            &frame.message,
        )?;
        let id = frame.id.into();
        S::command(DRIVER_NUM, SEND_MESSAGE, id, frame.len.into()).to_result()
    }

    pub fn send_message(frame: &Frame) -> Result<(), ErrorCode> {
        let upcall: Cell<Option<(u32,)>> = Cell::new(None);
        scope::<
            (
                AllowRo<_, DRIVER_NUM, { allow_ro::MESSAGE }>,
                Subscribe<S, DRIVER_NUM, { subscribe::MESSAGE_SENT }>,
            ),
            _,
            _,
        >(|handle| -> Result<(), ErrorCode> {
            let (allow_ro, subscribe_message_sent) = handle.split();
            S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::MESSAGE_SENT }>(
                subscribe_message_sent,
                &upcall,
            )?;
            Can::<S>::send_message_async(allow_ro, frame)?;
            // while upcall.get() == None {
            //     S::yield_wait();
            // }
            Ok(())
        })
    }

    pub fn start_receive<
        const BUFFER_LEN: usize,
        F: FnOnce(&Cell<Option<(u32,)>>, Handle<AllowRw<S, DRIVER_NUM, { allow_rw::MESSAGE_DST }>>),
    >(
        f: F,
    ) -> Result<(), ErrorCode> {
        let mut buffer = [0u8; BUFFER_LEN];
        let new_message: Cell<Option<(u32,)>> = Cell::new(None);
        scope::<
            (
                AllowRw<_, DRIVER_NUM, { allow_rw::MESSAGE }>,
                AllowRw<_, DRIVER_NUM, { allow_rw::MESSAGE_DST }>,
                Subscribe<S, DRIVER_NUM, { subscribe::MESSAGE_RECEIVED }>,
            ),
            _,
            _,
        >(|handle| -> Result<(), ErrorCode> {
            let (allow_handle, allow_handle_dst, subscribe_message_received) = handle.split();
            S::subscribe::<_, _, DefaultConfig, DRIVER_NUM, { subscribe::MESSAGE_RECEIVED }>(
                subscribe_message_received,
                &new_message,
            )?;

            S::allow_rw::<DefaultConfig, DRIVER_NUM, { allow_rw::MESSAGE }>(
                allow_handle,
                &mut buffer,
            )?;

            let r = S::command(DRIVER_NUM, START_RECEIVER, 0, 0).to_result();
            if let Err(ErrorCode::Already) = r {
                Ok(())
            } else {
                r
            }?;

            f(&new_message, allow_handle_dst);

            Ok(())
            // S::command(DRIVER_NUM, STOP_RECEIVER, 0, 0).to_result()
        })
    }

    pub fn read_messages<const BUFFER_LEN: usize>() -> Result<Frames<BUFFER_LEN>, ErrorCode> {
        let mut buffer = [0u8; BUFFER_LEN];

        // scope::<(AllowRw<_, DRIVER_NUM, { allow_rw::MESSAGE_DST }>,), _, _>(
        //     |handle| -> Result<(), ErrorCode> {
        //         let (allow_handle_dst,) = handle.split();
        //         S::allow_rw::<DefaultConfig, DRIVER_NUM, { allow_rw::MESSAGE_DST }>(
        //             allow_handle_dst,
        //             &mut buffer,
        //         )?;
        //         // S::command(DRIVER_NUM, READ_MESSAGES, 0, 0).to_result()
        //     },
        // )?;

        Ok(buffer.into())
    }

    pub fn set_bit_timing(
        segment1: u8,
        segment2: u8,
        propagation: u8,
        sync_jump_width: u8,
        baud_rate_prescaler: u8,
    ) -> Result<(), ErrorCode> {
        S::command(
            DRIVER_NUM,
            SET_TIMING,
            (segment1 as u32) << 24
                | (segment2 as u32) << 16
                | (sync_jump_width as u32) << 8
                | (baud_rate_prescaler as u32),
            propagation as u32,
        )
        .to_result()
    }

    pub fn state() -> Result<State, ErrorCode> {
        let r = S::command(DRIVER_NUM, STATE, 0, 0);
        if let Some(error) = r.get_failure() {
            Err(error)
        } else if let Some(state) = r.get_success_u32() {
            Ok(state.into())
        } else {
            Err(ErrorCode::Invalid)
        }
    }
}

/// The peripheral can be configured to work in the following modes:
#[derive(Debug, Copy, Clone)]
pub enum OperationMode {
    /// Loopback mode means that each message is transmitted on the
    /// TX channel and immediately received on the RX channel
    Loopback = 0,

    /// Monitoring mode means that the CAN peripheral sends only the recessive
    /// bits on the bus and cannot start a transmission, but can receive
    /// valid data frames and valid remote frames
    Monitoring = 1,

    /// Freeze mode means that no transmission or reception of frames is
    /// done
    Freeze = 2,

    /// Normal mode means that the transmission and reception of frames
    /// are available
    Normal = 3,
}

#[derive(Debug)]
pub struct Frame {
    pub id: Id,
    pub len: u8,
    pub message: [u8; STANDARD_CAN_PACKET_SIZE],
}

pub struct Frames<const BUFFER_LEN: usize> {
    pub messages: [u8; BUFFER_LEN],
    position: usize,
    num_messages: u8,
}

impl<const BUFFER_LEN: usize> Frames<BUFFER_LEN> {
    pub fn len(&self) -> usize {
        self.num_messages as usize
    }
}

impl<const BUFFER_LEN: usize> From<[u8; BUFFER_LEN]> for Frames<BUFFER_LEN> {
    fn from(buffer: [u8; BUFFER_LEN]) -> Frames<BUFFER_LEN> {
        assert!(BUFFER_LEN > 0);
        Frames {
            messages: buffer,
            position: 1,
            num_messages: buffer[0],
        }
    }
}

impl<const BUFFER_LEN: usize> Iterator for Frames<BUFFER_LEN> {
    type Item = Frame;

    fn next(&mut self) -> Option<Frame> {
        // if self.num_messages != 0 {
        //     // writeln!(Console::<libtock_runtime::TockSyscalls>::writer(), "[libtock rs can driver] num messages {} and position {}", self.num_messages, self.position).unwrap();
        // }
        if self.num_messages > 0 && self.position + MESSAGE_HEADER_LEN < BUFFER_LEN {
            let messages = &self.messages[self.position..];
            // id
            let id = (u32::from_le_bytes((messages[0..MESSAGE_ID_LEN]).try_into().unwrap())).into();

            // writeln!(Console::<libtock_runtime::TockSyscalls>::writer(), "{:?} ", id).unwrap();
            // len
            let len = messages[MESSAGE_HEADER_LEN];

            // writeln!(Console::<libtock_runtime::TockSyscalls>::writer(), "len is {} & messages len {}",len, messages.len()).unwrap();
            // data
            if (len as usize + MESSAGE_HEADER_LEN + 1) < messages.len() {
                let mut message = [0u8; STANDARD_CAN_PACKET_SIZE];
                message[0..len as usize].copy_from_slice(
                    &messages[MESSAGE_HEADER_LEN + 1..MESSAGE_HEADER_LEN + 1 + len as usize],
                );
                self.position = self.position + MESSAGE_HEADER_LEN + 1 + 8 as usize;
                self.num_messages = self.num_messages - 1;
                // writeln!(Console::<libtock_runtime::TockSyscalls>::writer(), "[libtock rs can driver] data is {:?}", &message[0..len as usize]).unwrap();
                Some(Frame { id, len, message })
            } else {
                None
            }
        } else {
            None
        }
    }
}

// 4 - 0-3
const MESSAGE_ID_LEN: usize = size_of::<u32>();
// 5 - 5
const MESSAGE_HEADER_LEN: usize = MESSAGE_ID_LEN + 1;
// 14
pub const MESSAGE_LEN: usize = MESSAGE_HEADER_LEN + STANDARD_CAN_PACKET_SIZE;

// -----------------------------------------------------------------------------
// Driver number and command IDs
// -----------------------------------------------------------------------------

const DRIVER_NUM: u32 = 0x20007;

// Command IDs
const EXISTS: u32 = 0;
const SET_BIT_RATE: u32 = 1;
const SET_MODE: u32 = 2;
const ENABLE: u32 = 3;
const DISABLE: u32 = 4;
const SEND_MESSAGE: u32 = 5;
const START_RECEIVER: u32 = 7;
const STOP_RECEIVER: u32 = 8;
const SET_TIMING: u32 = 9;
const READ_MESSAGES: u32 = 10;
const STATE: u32 = 11;

mod subscribe {
    pub const MESSAGE_SENT: u32 = 2;
    pub const MESSAGE_RECEIVED: u32 = 3;
}

mod allow_ro {
    pub const MESSAGE: u32 = 0;
}

mod allow_rw {
    pub const MESSAGE: u32 = 0;
    pub const MESSAGE_DST: u32 = 1;
}

pub const STANDARD_CAN_PACKET_SIZE: usize = 8;
