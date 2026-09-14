#![no_std]
#![no_main]

use core::fmt::Write;
use libtock::console::Console;
use libtock::crypto::{Hash, HashAlgorithm};
use libtock::runtime::{set_main, stack_size};

stack_size! {0x400}
set_main! {main}

fn main() {
    if let Err(e) = Hash::exists() {
        writeln!(Console::writer(), "HASH DRIVER ERROR: {e:?}").unwrap();
        return;
    }
    let mut console_writer = Console::writer();
    let input_buffer = [
        0x12, 0x34, 0x56, 0x78, 0x90, 0x98, 0x76, 0x54, 0x32, 0x12, 0x34, 0x56, 0x78, 0x90, 0x98,
        0x76, 0x54, 0x32, 0x12,
    ];
    let mut output_buffer: [u8; 32] = Default::default();
    let correct_output_buffer: [u8; 32] = [
        0x0a, 0x68, 0xe0, 0xa0, 0x19, 0xe2, 0x38, 0xb0, 0x21, 0xaf, 0x8b, 0xbc, 0x67, 0x36, 0x42,
        0xe1, 0xf8, 0x88, 0x93, 0xf4, 0xf7, 0xb6, 0x56, 0xcf, 0xa2, 0xaa, 0x25, 0x53, 0x6c, 0xc8,
        0x94, 0xb3,
    ];

    let _ = writeln!(console_writer, "---------------Hash Test---------------");
    match Hash::compute_sync(HashAlgorithm::SHA256, &input_buffer, &mut output_buffer) {
        Ok(()) => {
            let passed = output_buffer
                .iter()
                .zip(correct_output_buffer.iter())
                .enumerate()
                .all(|(idx, (&got, &expected))| {
                    let _ = writeln!(
                        console_writer,
                        "Index {}: got 0x{:02x}, expected: 0x{:02x}",
                        idx, got, expected
                    );
                    got == expected
                });
            if passed {
                let _ = writeln!(console_writer, "Verification successful!");
            } else {
                let _ = writeln!(console_writer, "Verification failed!");
            }
        }
        Err(e) => {
            let _ = writeln!(console_writer, "Error while computing hash {e:?}");
        }
    }
    return;
}
