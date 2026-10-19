#![no_std]

pub mod ecdsa;
pub mod hash;
pub mod hkdf;
pub mod hmac;

#[derive(Debug)]
pub enum HashAlgorithm {
    MD5 = 0,
    SHA1 = 1,
    SHA224 = 2,
    SHA256 = 3,
    SHA384 = 4,
    SHA512 = 5,
    SHA512_224 = 6,
    SHA512_256 = 7,
    SHA3_224 = 8,
    SHA3_256 = 9,
    SHA3_384 = 10,
    SHA3_512 = 11,
    SHAKE128 = 12,
    SHAKE256 = 13,
    SM3 = 14,
}

impl TryFrom<u32> for HashAlgorithm {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        use HashAlgorithm::*;
        Ok(match value {
            0 => MD5,
            1 => SHA1,
            2 => SHA224,
            3 => SHA256,
            4 => SHA384,
            5 => SHA512,
            6 => SHA512_224,
            7 => SHA512_256,
            8 => SHA3_224,
            9 => SHA3_256,
            10 => SHA3_384,
            11 => SHA3_512,
            12 => SHAKE128,
            13 => SHAKE256,
            14 => SM3,
            _ => return Err(()),
        })
    }
}
