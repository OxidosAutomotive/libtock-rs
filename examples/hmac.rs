#![no_std]
#![no_main]

use core::fmt::Write;
use libtock::console::Console;
use libtock::crypto::{HashAlgorithm, Hmac};
use libtock::runtime::{set_main, stack_size};

stack_size! {0x400}
set_main! {main}

fn main() {
    if let Err(e) = Hmac::exists() {
        writeln!(Console::writer(), "HMAC DRIVER ERROR: {e:?}").unwrap();
        return;
    }
    let mut console_writer = Console::writer();
    let input_buffer = [
        0x12, 0x34, 0x56, 0x78, 0x90, 0x98, 0x76, 0x54, 0x32, 0x12, 0x34, 0x56, 0x78, 0x90, 0x98,
        0x76, 0x54, 0x32, 0x12,
    ];
    let key_buffer = [
        0x12, 0x34, 0x56, 0x78, 0x76, 0x54, 0x32, 0x12, 0x34, 0x56, 0x78, 0x76, 0x54, 0x32,
    ];
    let mut output_buffer: [u8; 32] = Default::default();
    let correct_output_buffer: [u8; 32] = [
        0xe8, 0x19, 0xbf, 0xa1, 0xf4, 0x99, 0xa7, 0x89, 0x57, 0x44, 0x5b, 0xa8, 0xe0, 0x4e, 0x55,
        0xb0, 0xd3, 0xf5, 0x5e, 0xae, 0xd8, 0xf9, 0x1c, 0x8d, 0xe2, 0xb7, 0x56, 0x1e, 0xaa, 0xbf,
        0xff, 0x84,
    ];

    let _ = writeln!(console_writer, "---------------Hmac Test---------------");
    match Hmac::compute_sync(
        HashAlgorithm::SHA256,
        &input_buffer,
        &key_buffer,
        &mut output_buffer,
    ) {
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
