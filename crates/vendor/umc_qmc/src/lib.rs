//! vendored：上游 `umc-qmc` 的原样拷贝，只改了接入方式，算法一行没动。
//!
//! 这里不跟着本地 clippy 规则做风格改造——下次同步上游时，
//! 改动会和上游 diff 混在一起，反而看不清真实变化。
//! `chunks_exact_to_as_chunks` 是 Rust 1.99 才加的风格建议（建议换成 `as_chunks`），
//! 与正确性无关，因此在这一条上豁免。
#![allow(clippy::chunks_exact_to_as_chunks)]

use crate::v2_map::QMC2Map;
use crate::v2_rc4::cipher::QMC2RC4;
use thiserror::Error;

pub mod ekey;
pub mod footer;
pub mod v1;
pub mod v2_map;
pub mod v2_rc4;

#[derive(Error, Debug)]
pub enum QmcCryptoError {
    #[error("QMC V2/Map Cipher: Key is empty")]
    QMCV2MapKeyEmpty,

    #[error("EKey: {0}")]
    EKeyParseError(#[from] ekey::EKeyDecryptError),
}

#[derive(Debug, PartialEq, Clone)]
pub enum QMCv2Cipher {
    MapL(QMC2Map),
    RC4(QMC2RC4),
}

impl QMCv2Cipher {
    pub fn new<T>(key: T) -> Result<Self, QmcCryptoError>
    where
        T: AsRef<[u8]>,
    {
        let key = key.as_ref();
        let cipher = match key.len() {
            0 => Err(QmcCryptoError::QMCV2MapKeyEmpty)?,
            1..=300 => QMCv2Cipher::MapL(QMC2Map::new(key)?),
            _ => QMCv2Cipher::RC4(QMC2RC4::new(key)),
        };
        Ok(cipher)
    }

    pub fn new_from_ekey<T: AsRef<[u8]>>(ekey_str: T) -> Result<Self, QmcCryptoError> {
        let key = ekey::decrypt(ekey_str)?;
        Self::new(key)
    }

    pub fn decrypt<T>(&self, data: &mut T, offset: usize)
    where
        T: AsMut<[u8]> + ?Sized,
    {
        match self {
            QMCv2Cipher::MapL(cipher) => cipher.decrypt(data, offset),
            QMCv2Cipher::RC4(cipher) => cipher.decrypt(data, offset),
        }
    }
}

#[cfg(test)]
mod test {
    pub fn generate_key(len: usize) -> Vec<u8> {
        (1..=len).map(|i| i as u8).collect()
    }

    #[cfg(test)]
    pub fn generate_key_128() -> [u8; 128] {
        generate_key(128)
            .try_into()
            .expect("failed to make test key")
    }
}
