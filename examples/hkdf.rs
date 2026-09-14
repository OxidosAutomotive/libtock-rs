#![no_std]
#![no_main]

use core::fmt::Write;
use libtock::console::Console;
use libtock::crypto::{HashAlgorithm, Hkdf};
use libtock::runtime::{set_main, stack_size};

stack_size! {0x400}
set_main! {main}

fn main() {
    if let Err(e) = Hkdf::exists() {
        writeln!(Console::writer(), "HKDF DRIVER ERROR: {e:?}").unwrap();
        return;
    }
    let mut console_writer = Console::writer();
    let ikm_buffer = [
        0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b,
        0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b,
    ];
    let salt_buffer = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
    ];

    let info_buffer = [0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9];
    let mut prk_buffer: [u8; 32] = Default::default();
    let mut okm_buffer: [u8; 42] = [0u8; 42];
    let correct_output_buffer: [u8; 42] = [
        0x3c, 0xb2, 0x5f, 0x25, 0xfa, 0xac, 0xd5, 0x7a, 0x90, 0x43, 0x4f, 0x64, 0xd0, 0x36, 0x2f,
        0x2a, 0x2d, 0x2d, 0x0a, 0x90, 0xcf, 0x1a, 0x5a, 0x4c, 0x5d, 0xb0, 0x2d, 0x56, 0xec, 0xc4,
        0xc5, 0xbf, 0x34, 0x00, 0x72, 0x08, 0xd5, 0xb8, 0x87, 0x18, 0x58, 0x65,
    ];

    let _ = writeln!(console_writer, "---------------Hkdf Test---------------");
    match Hkdf::compute_sync(
        HashAlgorithm::SHA256,
        &ikm_buffer,
        &mut prk_buffer,
        &mut okm_buffer,
        Some(&salt_buffer),
        Some(&info_buffer),
    ) {
        Ok(()) => {
            let passed = okm_buffer
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
