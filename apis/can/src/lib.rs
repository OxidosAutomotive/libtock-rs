#![no_std]

use core::cell::Cell;

use libtock_platform::{
    share::scope, share::Handle, AllowRo, AllowRw, DefaultConfig, ErrorCode, Subscribe, Syscalls,
};

pub struct Can<S: Syscalls>(S);

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum State {
    Uninit = 0,
    Running = 1,
    Sleep = 2,
    Stopped = 3,
    BusOff = 4,
}

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
            Id::Extended(value & 0x1fff_ffff)
        } else {
            Id::Standard((value & 0x7fff) as u16)
        }
    }
}

impl From<Id> for u32 {
    fn from(id: Id) -> u32 {
        match id {
            Id::Standard(id) => id as u32,
            Id::Extended(id) => id,
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
        F: FnOnce(
            &Cell<Option<(u32, u32, u32)>>,
            Handle<AllowRw<S, DRIVER_NUM, { allow_rw::MESSAGE_DST }>>,
        ),
    >(
        f: F,
    ) -> Result<(), ErrorCode> {
        let mut buffer = [0u8; CANFRAME_SIZE * CANFRAME_MAX_NUM + UNREAD_COUNTER_SIZE];
        let new_message: Cell<Option<(u32, u32, u32)>> = Cell::new(None);
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

            S::command(DRIVER_NUM, STOP_RECEIVER, 0, 0).to_result()
        })
    }

    pub fn read_messages() -> Result<Frames, ErrorCode> {
        let mut buffer = [0u8; UNREAD_COUNTER_SIZE + CANFRAME_SIZE * CANFRAME_MAX_NUM];
        scope::<(AllowRw<_, DRIVER_NUM, { allow_rw::MESSAGE_DST }>,), _, _>(
            |handle| -> Result<(), ErrorCode> {
                let (allow_handle_dst,) = handle.split();
                S::allow_rw::<DefaultConfig, DRIVER_NUM, { allow_rw::MESSAGE_DST }>(
                    allow_handle_dst,
                    &mut buffer,
                )?;
                S::command(DRIVER_NUM, READ_MESSAGES, 0, 0).to_result()
            },
        )?;
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

    /// returns the last received frame with given Id. The kernel will save the last received frame for each Id it is configured to do so in software mailboxes
    /// Returns:
    ///     * Ok(Frame, read_counter, flags) = { - a frame object with Id, length, data[0:7]
    ///                                          - how many times this frame was read, the counter is restarted if a new frame with the same Id is received, max 255
    ///                                          - flag bits for previouse frame being overwritten without being read etc
    ///     * Err(ErrorCode::NOMEM) = a frame with that Id was not received previously
    ///     * Err(ErrorCode::INVAL) = the given Id is not part of the select Id
    ///     * Err(ErrorCode::BadRVal) = the kernel returned unusual data
    pub fn read_special_frame(id: &Id) -> Result<(Frame, u8, u8), ErrorCode> {
        let result = S::command(DRIVER_NUM, READ_SPECIAL_FRAME, u32::from(*id), 0);
        let returned = result
            .get_success_u32_u64()
            .ok_or(result.get_failure().unwrap_or(ErrorCode::BadRVal))?;

        let status = u32::to_be_bytes(returned.0); // [read_counter; length; flags; 0]

        let frame = Frame {
            id: *id,
            len: status[1],
            message: u64::to_be_bytes(returned.1),
        };
        Ok((frame, status[0], status[2]))
    }

    /// returns the last received frame with given Id just the first time it is read, the frame is further considered stale
    /// Returns:
    ///     * Ok(Frame) = the first time a frame is read (with given Id)
    ///     * Err(ErrorCode::Already) = the frame was not updated
    ///     * Err(ErrorCode::NOMEM) = a frame with that Id was not received previously
    ///     * Err(_) = as the above function (`read_special_frame`)
    pub fn read_new_special_frame(id: &Id) -> Result<Frame, ErrorCode> {
        let (frame, read_counter, _) = Self::read_special_frame(id)?;
        if read_counter == 0 {
            Ok(frame)
        } else {
            Err(ErrorCode::Already)
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

const CANFRAME_SIZE: usize = 14;
const CANFRAME_MAX_NUM: usize = 3;
const UNREAD_COUNTER_SIZE: usize = 1;

const CANFRAME_HEADER_SIZE: usize = 6;
const CANFRAME_DATA_SIZE: usize = 8;

#[derive(Debug)]
pub struct Frame {
    pub id: Id,
    pub len: u8,
    pub message: [u8; STANDARD_CAN_PACKET_SIZE],
}

#[derive(Debug)]
pub struct Frames {
    pub buffers: [u8; CANFRAME_MAX_NUM * CANFRAME_SIZE],
    index: usize,
    length: u8,
}

impl Frames {
    pub fn len(&self) -> usize {
        self.length as usize
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

impl From<[u8; UNREAD_COUNTER_SIZE + CANFRAME_MAX_NUM * CANFRAME_SIZE]> for Frames {
    fn from(buffer: [u8; UNREAD_COUNTER_SIZE + CANFRAME_MAX_NUM * CANFRAME_SIZE]) -> Frames {
        // Get only the CAN frames from the buffer.
        let mut messages = [0u8; CANFRAME_MAX_NUM * CANFRAME_SIZE];
        messages.copy_from_slice(&buffer[UNREAD_COUNTER_SIZE..]);

        Frames {
            buffers: messages,
            index: 0,
            length: buffer[0],
        }
    }
}

impl Iterator for Frames {
    type Item = Frame;

    fn next(&mut self) -> Option<Frame> {
        if self.length > 0 && self.index + CANFRAME_SIZE <= CANFRAME_MAX_NUM * CANFRAME_SIZE {
            // The frames the current cursor is pointing at.
            let frame = &self.buffers[self.index..(self.index + CANFRAME_SIZE)];

            // Compute the CAN Frame ID.
            let mut id_bytes = [0u8; 4];
            id_bytes.copy_from_slice(&frame[0..4]);
            let id: Id = u32::from_be_bytes(id_bytes).into();

            // Get the length. (Each packet still has reserved 8 bytes. (TODO: Remove)
            let len = frame[4];

            // The "next" item will actually be the the one of the current index.
            let mut next_frame_data = [0u8; CANFRAME_DATA_SIZE];
            next_frame_data.copy_from_slice(&frame[CANFRAME_HEADER_SIZE..]);

            self.index += CANFRAME_SIZE;

            Some(Frame {
                id,
                len,
                message: next_frame_data,
            })
        } else {
            None
        }
    }
}

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
const READ_SPECIAL_FRAME: u32 = 12;

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
