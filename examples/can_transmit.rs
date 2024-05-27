#![no_main]
#![no_std]

use core::fmt::Write;
use libtock::alarm::{Alarm, Milliseconds};
use libtock::can::can::{Frame, OperationMode, State};
use libtock::can::Can;
use libtock::console::Console;
use libtock::runtime::{set_main, stack_size};

set_main! {main}
stack_size! {0x2000}

fn main() {
    writeln!(Console::writer(), "\nHello, CAN!").unwrap();

    if let Ok(State::Running) = Can::state() {
        writeln!(Console::writer(), "Can is already running :(").unwrap();
    } else {
        match Can::set_baudrate(100_000) {
            Ok(_) => writeln!(Console::writer(), "Can baud rate set!").unwrap(),
            Err(_) => writeln!(Console::writer(), "Error at setting the baud rate!").unwrap(),
        };
        match Can::set_operation_mode(OperationMode::Normal) {
            Ok(_) => writeln!(Console::writer(), "Can operation set to normal!").unwrap(),
            Err(_) => writeln!(Console::writer(), "Error at setting the operation mode!").unwrap(),
        }
    }

    // Mock CAN messages.
    let can_messages = [
        (5u8, [0xA0u8, 0xB0, 0xC0, 0xD0, 0xE0, 0x00, 0x00, 0x00]),
        (6, [0xA1u8, 0xB1, 0xC1, 0xD1, 0xE1, 0xF1, 0x00, 0x00]),
        (7, [0xA2u8, 0xB2, 0xC2, 0xD2, 0xE2, 0xF2, 0x12, 0x00]),
        (8, [0xA3u8, 0xB3, 0xC3, 0xD3, 0xE3, 0xF3, 0x13, 0x23]),
        (8, [0xA4u8, 0xB4, 0xC4, 0xD4, 0xE4, 0xF4, 0x14, 0x24]),
        (8, [0xA5u8, 0xB5, 0xC5, 0xD5, 0xE5, 0xF5, 0x15, 0x25]),
    ];

    for i in 0..6 {
        let frame = Frame {
            id: libtock::can::can::Id::Standard(0x00A1u16),
            len: can_messages[i].0,
            message: can_messages[i].1,
        };

        match Can::send_message(&frame) {
            Ok(_) => writeln!(
                Console::writer(),
                "Message with frame id {:?} sent on bus!",
                frame.id
            )
            .unwrap(),
            Err(err) => {
                writeln!(Console::writer(), "Error at sending the message: {:?}", err).unwrap()
            }
        }
        Alarm::sleep_for(Milliseconds(2500)).unwrap();
    }
}
