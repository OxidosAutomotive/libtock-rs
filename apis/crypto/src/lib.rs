#![no_std]

pub mod hash;
pub mod hkdf;
pub mod hmac;

pub enum HashAlgorithm {
    MD5 = 0,
    SHA1 = 1,
    SHA224 = 2,
    SHA256 = 3,
    SHA384 = 4,
    SHA512 = 5,
    SHA512_224 = 6,
    SHA512_256 = 7,
}
