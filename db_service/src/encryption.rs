use cocoon::Error as CocoonError;
use std::fmt::Debug;
use std::{error::Error, fmt};

use cocoon::Cocoon;

use shared_lib::env_utils::get_db_encryption_key;

#[derive(Debug)]
pub enum EncryptError {
    CocoonError(CocoonError),
    Utf8Error(std::string::FromUtf8Error),
}

impl fmt::Display for EncryptError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EncryptError::CocoonError(e) => write!(f, "Cocoon error: {e:?}"),
            EncryptError::Utf8Error(e) => write!(f, "UTF8 error: {e:?}"),
        }
    }
}

impl Error for EncryptError {}

impl From<CocoonError> for EncryptError {
    fn from(err: CocoonError) -> EncryptError {
        EncryptError::CocoonError(err)
    }
}

impl From<std::string::FromUtf8Error> for EncryptError {
    fn from(err: std::string::FromUtf8Error) -> EncryptError {
        EncryptError::Utf8Error(err)
    }
}

pub fn encrypt(value: String) -> Result<Vec<u8>, CocoonError> {
    let encryption_key = get_db_encryption_key();
    let mut cocoon = Cocoon::new(encryption_key.as_bytes());
    let encrypted = cocoon.wrap(value.as_bytes())?;
    Ok(encrypted)
}

pub fn decrypt(value: Vec<u8>) -> Result<String, EncryptError> {
    let encryption_key = get_db_encryption_key();
    let cocoon = Cocoon::new(encryption_key.as_bytes());
    let decrypted = cocoon.unwrap(&value)?;
    let decrypted = String::from_utf8(decrypted)?;
    Ok(decrypted)
}
