//! AES-128-CBC with a zero IV and PKCS7 padding, as used by the game's
//! save-encryption routine.

use aes::Aes128;
use cbc::{Decryptor, Encryptor};
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};

pub const SAVE_KEY: &[u8; 16] = b"W9JU3JASLM7765D2";
pub const SAVE_IV: &[u8; 16] = &[0u8; 16];

type Aes128CbcEnc = Encryptor<Aes128>;
type Aes128CbcDec = Decryptor<Aes128>;

pub fn encrypt_save(plaintext: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; plaintext.len() + 16];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let ct = Aes128CbcEnc::new(SAVE_KEY.into(), SAVE_IV.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .expect("buffer has room for a full block of padding");
    ct.to_vec()
}

pub fn decrypt_save(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err("save file is not a whole number of AES blocks".into());
    }
    let mut buf = ciphertext.to_vec();
    let pt = Aes128CbcDec::new(SAVE_KEY.into(), SAVE_IV.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| format!("decrypt/unpad failed: {e}"))?;
    Ok(pt.to_vec())
}
