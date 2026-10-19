#![no_std]
#![no_main]

use core::fmt::Write;
use libtock::console::Console;
use libtock::crypto::{Ecdsa};
use libtock::runtime::{set_main, stack_size};

stack_size! {0x400}
set_main! {main}

fn main() {
    if let Err(e) = Ecdsa::exists() {
        writeln!(Console::writer(), "ECDSA DRIVER ERROR: {e:?}").unwrap();
        return;
    }
    let mut console_writer = Console::writer();
    let hash_to_sign: [u8; 32] = [
        0xaf, 0x2b, 0xdb, 0xe1, 0xaa, 0x9b, 0x6e, 0xc1, 0xe2, 0xad, 0xe1, 0xd6, 0x94, 0xf4, 0x1f,
        0xc7, 0x1a, 0x83, 0x1d, 0x02, 0x68, 0xe9, 0x89, 0x15, 0x62, 0x11, 0x3d, 0x8a, 0x62, 0xad,
        0xd1, 0xbf,
    ];
    let mut signature: [u8; 64] = [0; 64];
    let correct_signature: [u8; 64] = [
        0xEF, 0xD4, 0x8B, 0x2A, 0xAC, 0xB6, 0xA8, 0xFD, 0x11, 0x40, 0xDD, 0x9C, 0xD4, 0x5E, 0x81,
        0xD6, 0x9D, 0x2C, 0x87, 0x7B, 0x56, 0xAA, 0xF9, 0x91, 0xC3, 0x4D, 0x0E, 0xA8, 0x4E, 0xAF,
        0x37, 0x16, 0xF7, 0xCB, 0x1C, 0x94, 0x2D, 0x65, 0x7C, 0x41, 0xD4, 0x36, 0xC7, 0xA1, 0xB6,
        0xE2, 0x9F, 0x65, 0xF3, 0xE9, 0x00, 0xDB, 0xB9, 0xAF, 0xF4, 0x06, 0x4D, 0xC4, 0xAB, 0x2F,
        0x84, 0x3A, 0xCD, 0xA8,
    ];

    let _ = writeln!(console_writer, "---------------Ecdsa Test---------------");
    match Ecdsa::compute_sync(&hash_to_sign, &mut signature) {
        Ok(()) => {
            let passed = signature
                .iter()
                .zip(correct_signature.iter())
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
            let _ = writeln!(console_writer, "Error while computing signature {e:?}");
        }
    }
}
