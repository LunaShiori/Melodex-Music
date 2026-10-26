//! 酷我音乐自定义 DES（kwDES）加解密模块。
//!
//! 酷我音乐客户端使用一种非标准的 DES 变体（`kwDES`）来加解密：
//! - 使用 LSB-first 位序，与标准 DES 相反。
//! - S-box 采用字节对齐分组。
//! - 因此标准 `des` crate 无法兼容此算法。
//!
//! 本实现移植自 HotDownloader（Apache-2.0，github.com/lerdb/HotDownloader），
//! 仅用于解密酷我接口返回的 `ekey` 字段，得到 `.mflac`/`.mgg` 的解密密钥。

use base64::{engine::general_purpose::STANDARD, Engine as _};

/// 默认密钥
pub const DEFAULT_KEY: &[u8; 8] = b"ylzsxkwm";

// ============================================================================
// 常量表
// ============================================================================

/// E 扩展置换表
const EXPANSION: [i16; 64] = [
    31, 0, 1, 2, 3, 4, -1, -1, 3, 4, 5, 6, 7, 8, -1, -1, 7, 8, 9, 10, 11, 12, -1, -1, 11, 12, 13,
    14, 15, 16, -1, -1, 15, 16, 17, 18, 19, 20, -1, -1, 19, 20, 21, 22, 23, 24, -1, -1, 23, 24, 25,
    26, 27, 28, -1, -1, 27, 28, 29, 30, 31, 30, -1, -1,
];

/// 初始置换 IP
const INITIAL_PERMUTATION: [i16; 64] = [
    57, 49, 41, 33, 25, 17, 9, 1, 59, 51, 43, 35, 27, 19, 11, 3, 61, 53, 45, 37, 29, 21, 13, 5, 63,
    55, 47, 39, 31, 23, 15, 7, 56, 48, 40, 32, 24, 16, 8, 0, 58, 50, 42, 34, 26, 18, 10, 2, 60, 52,
    44, 36, 28, 20, 12, 4, 62, 54, 46, 38, 30, 22, 14, 6,
];

/// 末置换 IP-1
const INVERSE_PERMUTATION: [i16; 64] = [
    39, 7, 47, 15, 55, 23, 63, 31, 38, 6, 46, 14, 54, 22, 62, 30, 37, 5, 45, 13, 53, 21, 61, 29,
    36, 4, 44, 12, 52, 20, 60, 28, 35, 3, 43, 11, 51, 19, 59, 27, 34, 2, 42, 10, 50, 18, 58, 26,
    33, 1, 41, 9, 49, 17, 57, 25, 32, 0, 40, 8, 48, 16, 56, 24,
];

/// 每轮左移位数
const SHIFT_SCHEDULE: [u8; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

/// 左移掩码辅助
const SHIFT_MASKS: [u64; 3] = [0, 0x100001, 0x300003];

/// P 置换
const PERMUTATION: [i16; 32] = [
    15, 6, 19, 20, 28, 11, 27, 16, 0, 14, 22, 25, 4, 17, 30, 9, 1, 7, 23, 13, 31, 26, 2, 8, 18, 12,
    29, 5, 21, 10, 3, 24,
];

/// PC-1 置换
const PC1: [i16; 56] = [
    56, 48, 40, 32, 24, 16, 8, 0, 57, 49, 41, 33, 25, 17, 9, 1, 58, 50, 42, 34, 26, 18, 10, 2, 59,
    51, 43, 35, 62, 54, 46, 38, 30, 22, 14, 6, 61, 53, 45, 37, 29, 21, 13, 5, 60, 52, 44, 36, 28,
    20, 12, 4, 27, 19, 11, 3,
];

/// PC-2 置换
const PC2: [i16; 64] = [
    13, 16, 10, 23, 0, 4, -1, -1, 2, 27, 14, 5, 20, 9, -1, -1, 22, 18, 11, 3, 25, 7, -1, -1, 15, 6,
    26, 19, 12, 1, -1, -1, 40, 51, 30, 36, 46, 54, -1, -1, 29, 39, 50, 44, 32, 47, -1, -1, 43, 48,
    38, 55, 33, 52, -1, -1, 45, 41, 49, 35, 28, 31, -1, -1,
];

/// S-box 矩阵（8 组 × 64 字节）
const S_BOXES: [[u8; 64]; 8] = [
    [
        14, 4, 3, 15, 2, 13, 5, 3, 13, 14, 6, 9, 11, 2, 0, 5, 4, 1, 10, 12, 15, 6, 9, 10, 1, 8, 12,
        7, 8, 11, 7, 0, 0, 15, 10, 5, 14, 4, 9, 10, 7, 8, 12, 3, 13, 1, 3, 6, 15, 12, 6, 11, 2, 9,
        5, 0, 4, 2, 11, 14, 1, 7, 8, 13,
    ],
    [
        15, 0, 9, 5, 6, 10, 12, 9, 8, 7, 2, 12, 3, 13, 5, 2, 1, 14, 7, 8, 11, 4, 0, 3, 14, 11, 13,
        6, 4, 1, 10, 15, 3, 13, 12, 11, 15, 3, 6, 0, 4, 10, 1, 7, 8, 4, 11, 14, 13, 8, 0, 6, 2, 15,
        9, 5, 7, 1, 10, 12, 14, 2, 5, 9,
    ],
    [
        10, 13, 1, 11, 6, 8, 11, 5, 9, 4, 12, 2, 15, 3, 2, 14, 0, 6, 13, 1, 3, 15, 4, 10, 14, 9, 7,
        12, 5, 0, 8, 7, 13, 1, 2, 4, 3, 6, 12, 11, 0, 13, 5, 14, 6, 8, 15, 2, 7, 10, 8, 15, 4, 9,
        11, 5, 9, 0, 14, 3, 10, 7, 1, 12,
    ],
    [
        7, 10, 1, 15, 0, 12, 11, 5, 14, 9, 8, 3, 9, 7, 4, 8, 13, 6, 2, 1, 6, 11, 12, 2, 3, 0, 5,
        14, 10, 13, 15, 4, 13, 3, 4, 9, 6, 10, 1, 12, 11, 0, 2, 5, 0, 13, 14, 2, 8, 15, 7, 4, 15,
        1, 10, 7, 5, 6, 12, 11, 3, 8, 9, 14,
    ],
    [
        2, 4, 8, 15, 7, 10, 13, 6, 4, 1, 3, 12, 11, 7, 14, 0, 12, 2, 5, 9, 10, 13, 0, 3, 1, 11, 15,
        5, 6, 8, 9, 14, 14, 11, 5, 6, 4, 1, 3, 10, 2, 12, 15, 0, 13, 2, 8, 5, 11, 8, 0, 15, 7, 14,
        9, 4, 12, 7, 10, 9, 1, 13, 6, 3,
    ],
    [
        12, 9, 0, 7, 9, 2, 14, 1, 10, 15, 3, 4, 6, 12, 5, 11, 1, 14, 13, 0, 2, 8, 7, 13, 15, 5, 4,
        10, 8, 3, 11, 6, 10, 4, 6, 11, 7, 9, 0, 6, 4, 2, 13, 1, 9, 15, 3, 8, 15, 3, 1, 14, 12, 5,
        11, 0, 2, 12, 14, 7, 5, 10, 8, 13,
    ],
    [
        4, 1, 3, 10, 15, 12, 5, 0, 2, 11, 9, 6, 8, 7, 6, 9, 11, 4, 12, 15, 0, 3, 10, 5, 14, 13, 7,
        8, 13, 14, 1, 2, 13, 6, 14, 9, 4, 1, 2, 14, 11, 13, 5, 0, 1, 10, 8, 3, 0, 11, 3, 5, 9, 4,
        15, 2, 7, 8, 12, 15, 10, 7, 6, 12,
    ],
    [
        13, 7, 10, 0, 6, 9, 5, 15, 8, 4, 3, 10, 11, 14, 12, 5, 2, 11, 9, 6, 15, 12, 0, 3, 4, 1, 14,
        13, 1, 2, 7, 8, 1, 2, 12, 15, 10, 4, 0, 3, 13, 14, 6, 9, 7, 8, 9, 6, 15, 1, 5, 12, 3, 10,
        14, 5, 8, 7, 11, 0, 4, 13, 2, 11,
    ],
];

#[inline]
fn permute(data: u64, mapper: &[i16]) -> u64 {
    let mut ret = 0u64;
    for (i, &v) in mapper.iter().enumerate() {
        if v < 0 {
            continue;
        }
        if (data & (1u64 << v)) != 0 {
            ret |= 1u64 << i;
        }
    }
    ret
}

#[inline]
fn derive_keys(key_int: u64, round_keys: &mut [u64; 16]) {
    let mut key56 = permute(key_int, &PC1);
    for i in 0..16 {
        let shift = SHIFT_SCHEDULE[i];
        let mask = SHIFT_MASKS[shift as usize];
        key56 = ((key56 & mask) << (28 - shift as u32)) | ((key56 & !mask) >> shift as u32);
        round_keys[i] = permute(key56, &PC2);
    }
}

#[inline]
fn f(half_block: u64, subkey: u64) -> u64 {
    let expanded = permute(half_block, &EXPANSION);
    let mixed = expanded ^ subkey;
    let mut s_output = 0u64;
    for i in (0..8u8).rev() {
        let six_bits = ((mixed >> (i * 8)) & 0x3F) as usize;
        s_output = (s_output << 4) | u64::from(S_BOXES[i as usize][six_bits]);
    }
    permute(s_output, &PERMUTATION)
}

#[inline]
fn encrypt_block(block: u64, round_keys: &[u64; 16]) -> u64 {
    let permuted = permute(block, &INITIAL_PERMUTATION);
    let mut left = permuted & 0xFFFF_FFFF;
    let mut right = (permuted >> 32) & 0xFFFF_FFFF;
    for &subkey in round_keys.iter() {
        let new_right = left ^ f(right, subkey);
        left = right;
        right = new_right;
    }
    let combined = (left << 32) | right;
    permute(combined, &INVERSE_PERMUTATION)
}

/// 对数据进行 kwDES 加/解密（ECB 模式，零填充）。
pub fn des_crypt(data: &[u8], key: &[u8; 8], decrypt: bool) -> Vec<u8> {
    let key_int = u64::from_le_bytes(*key);
    let mut round_keys = [0u64; 16];
    derive_keys(key_int, &mut round_keys);
    if decrypt {
        round_keys.reverse();
    }
    let block_count = data.len().div_ceil(8);
    let mut output = vec![0u8; block_count * 8];
    for (i, chunk) in data.chunks(8).enumerate() {
        let mut block_bytes = [0u8; 8];
        block_bytes[..chunk.len()].copy_from_slice(chunk);
        let block_int = u64::from_le_bytes(block_bytes);
        let processed = encrypt_block(block_int, &round_keys);
        output[i * 8..(i + 1) * 8].copy_from_slice(&processed.to_le_bytes());
    }
    output
}

/// kwDES 解密 + Base64 解码，返回去尾部 `\0` 填充的 UTF-8 字符串。
pub fn base64_decrypt(msg: &str, key: &[u8; 8]) -> Result<String, String> {
    let decoded = STANDARD
        .decode(msg)
        .map_err(|e| format!("Base64 解码失败: {e}"))?;
    let decrypted = des_crypt(&decoded, key, true);
    let end = decrypted
        .iter()
        .rposition(|&b| b != 0)
        .map(|p| p + 1)
        .unwrap_or(0);
    String::from_utf8(decrypted[..end].to_vec()).map_err(|e| format!("UTF-8 解码失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_via_encrypt() {
        // 用已知向量验证解密：base64_encrypt("12345") == "6kgYe3imgc4="
        // 这里直接验证 base64_decrypt 能还原
        let decrypted = base64_decrypt("6kgYe3imgc4=", DEFAULT_KEY).expect("解密失败");
        assert_eq!(decrypted, "12345");
    }
}
