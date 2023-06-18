use log::warn;
use pimalaya_secret::Secret;
use std::{
    io,
    ops::{Deref, DerefMut},
};
use thiserror::Error;

use crate::Result;

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot get password from user")]
    GetFromUserError(#[source] io::Error),
    #[error("cannot get password from global keyring")]
    GetFromKeyringError(#[source] pimalaya_secret::Error),
    #[error("cannot save password into global keyring")]
    SetIntoKeyringError(#[source] pimalaya_secret::Error),
    #[error("cannot delete password from global keyring")]
    DeleteError(#[source] pimalaya_secret::Error),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PasswdConfig {
    pub passwd: Secret,
}

impl Deref for PasswdConfig {
    type Target = Secret;

    fn deref(&self) -> &Self::Target {
        &self.passwd
    }
}

impl DerefMut for PasswdConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.passwd
    }
}

impl PasswdConfig {
    pub fn reset(&self) -> Result<()> {
        self.delete_keyring_entry_secret()
            .map_err(Error::DeleteError)?;
        Ok(())
    }

    pub fn configure(&self, get_passwd: impl Fn() -> io::Result<String>) -> Result<()> {
        match self.find() {
            Ok(None) => {
                warn!("cannot find imap password from keyring, setting it");
                let passwd = get_passwd().map_err(Error::GetFromUserError)?;
                self.set_keyring_entry_secret(passwd)
                    .map_err(Error::SetIntoKeyringError)?;
                Ok(())
            }
            Ok(_) => Ok(()),
            Err(err) => Ok(Err(Error::GetFromKeyringError(err))?),
        }
    }
}