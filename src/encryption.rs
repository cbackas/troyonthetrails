use cocoon::Error as CocoonError;
use std::fmt::Debug;
use std::{error::Error, fmt};

use cocoon::Cocoon;

use crate::env_utils::get_db_encryption_key;

#[derive(Debug)]
pub enum EncryptError {
    CocoonError(CocoonError),
    Utf8Error(std::string::FromUtf8Error),
}

impl fmt::Display for EncryptError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            e => std::fmt::Debug::fmt(&e, f),
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

pub fn encrypt(value: String) -> Result<String, EncryptError> {
    let encryption_key = get_db_encryption_key();
    let mut cocoon = Cocoon::new(&encryption_key.as_bytes());
    let encrypted = cocoon.wrap(value.as_bytes())?;
    let encrypted_string = String::from_utf8(encrypted)?;
    Ok(encrypted_string)
}

pub fn decrypt(value: String) -> Result<String, EncryptError> {
    let encryption_key = get_db_encryption_key();
    let cocoon = Cocoon::new(&encryption_key.as_bytes());
    let decrypted = cocoon.unwrap(&value.as_bytes())?;
    let decrypted = String::from_utf8(decrypted)?;
    Ok(decrypted)
}
