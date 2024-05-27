#![no_main]
#![no_std]

use core::fmt::Write;
use libtock::can::can::{OperationMode, State};
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

    Can::start_receive::<_>(|new_message, _| loop {
        if new_message.get().is_some() {
            let mut frames = Can::read_messages().unwrap();

            if let Some(frame) = frames.next() {
                writeln!(Console::writer(), "Received frame: {:?}", frame).unwrap();
            }
        }
    })
    .unwrap();
}
