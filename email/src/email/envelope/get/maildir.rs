use async_trait::async_trait;
use log::{info, trace};

use crate::{email::error::Error, envelope::Id, maildir::MaildirContextSync};

use super::{Envelope, GetEnvelope};

#[derive(Clone)]
pub struct GetMaildirEnvelope {
    ctx: MaildirContextSync,
}

impl GetMaildirEnvelope {
    pub fn new(ctx: &MaildirContextSync) -> Self {
        Self { ctx: ctx.clone() }
    }

    pub fn new_boxed(ctx: &MaildirContextSync) -> Box<dyn GetEnvelope> {
        Box::new(Self::new(ctx))
    }

    pub fn some_new_boxed(ctx: &MaildirContextSync) -> Option<Box<dyn GetEnvelope>> {
        Some(Self::new_boxed(ctx))
    }
}

#[async_trait]
impl GetEnvelope for GetMaildirEnvelope {
    async fn get_envelope(&self, folder: &str, id: &Id) -> crate::Result<Envelope> {
        info!("getting maildir envelope {id} from folder {folder}");

        let session = self.ctx.lock().await;
        let mdir = session.get_maildir_from_folder_name(folder)?;

        let envelope: Envelope =
            Envelope::from_mdir_entry(mdir.find(&id.to_string()).ok_or_else(|| {
                Error::GetEnvelopeMaildirError(mdir.path().to_owned(), id.clone())
            })?);
        trace!("maildir envelope: {envelope:#?}");

        Ok(envelope)
    }
}
