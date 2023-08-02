//! Module dedicated to PGP sign.

use pgp::{crypto::hash::HashAlgorithm, Message, SignedSecretKey};
use thiserror::Error;
use tokio::task;

use crate::Result;

/// Errors related to PGP.
#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot sign pgp message")]
    SignMessageError(#[source] pgp::errors::Error),
    #[error("cannot export signed pgp message as armored string")]
    ExportSignedMessageToArmoredBytesError(#[source] pgp::errors::Error),
}

/// Signs data using the given private key.
pub async fn sign(data: Vec<u8>, skey: SignedSecretKey, passwd: impl ToString) -> Result<Vec<u8>> {
    let passwd = passwd.to_string();

    task::spawn_blocking(move || {
        let msg = Message::new_literal_bytes("", &data)
            .sign(&skey, || passwd, HashAlgorithm::SHA1)
            .map_err(Error::SignMessageError)?;

        let sig = msg
            .into_signature()
            .to_armored_bytes(None)
            .map_err(Error::ExportSignedMessageToArmoredBytesError)?;

        Ok(sig)
    })
    .await?
}
