#![no_std]

pub mod ecdsa;
pub mod hash;
pub mod hkdf;
pub mod hmac;

pub trait DigestAlgorithm {
    const COMMAND_NUM: u32;
    const DIGEST_LEN: usize;
    const BLOCK_LEN: usize;

    type Output: AsRef<[u8]> + AsMut<[u8]> + Sized;

    const ZEROED: Self::Output;
}

macro_rules! digest_algorithm {
    ($name:ident, $command_num:expr, $digest_len:expr, $block_len:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name;

        impl DigestAlgorithm for $name {
            const COMMAND_NUM: u32 = $command_num;
            const DIGEST_LEN: usize = $digest_len;
            const BLOCK_LEN: usize = $block_len;
            type Output = [u8; $digest_len];
            const ZEROED: Self::Output = [0u8; $digest_len];
        }
    };
}

pub mod algos {
    use crate::DigestAlgorithm;
    digest_algorithm!(Md5, 0, 16, 512 >> 3);
    digest_algorithm!(Sha1, 1, 20, 512 >> 3);
    digest_algorithm!(Sha224, 2, 28, 512 >> 3);
    digest_algorithm!(Sha256, 3, 32, 512 >> 3);
    digest_algorithm!(Sha384, 4, 48, 1024 >> 3);
    digest_algorithm!(Sha512, 5, 64, 1024 >> 3);
    digest_algorithm!(Sha512_224, 6, 28, 1024 >> 3);
    digest_algorithm!(Sha512_256, 7, 32, 1024 >> 3);
    digest_algorithm!(Sha3_224, 8, 28, 1152 >> 3);
    digest_algorithm!(Sha3_256, 9, 32, 1088 >> 3);
    digest_algorithm!(Sha3_384, 10, 48, 832 >> 3);
    digest_algorithm!(Sha3_512, 11, 64, 576 >> 3);
    digest_algorithm!(Sm3, 14, 32, 512 >> 3);
}
