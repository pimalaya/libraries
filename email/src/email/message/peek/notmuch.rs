use async_trait::async_trait;
use log::info;
use std::fs;

use crate::{email::error::Error, envelope::Id, notmuch::NotmuchContextSync};

use super::{Messages, PeekMessages};

#[derive(Clone)]
pub struct PeekNotmuchMessages {
    ctx: NotmuchContextSync,
}

impl PeekNotmuchMessages {
    pub fn new(ctx: &NotmuchContextSync) -> Self {
        Self { ctx: ctx.clone() }
    }

    pub fn new_boxed(ctx: &NotmuchContextSync) -> Box<dyn PeekMessages> {
        Box::new(Self::new(ctx))
    }

    pub fn some_new_boxed(ctx: &NotmuchContextSync) -> Option<Box<dyn PeekMessages>> {
        Some(Self::new_boxed(ctx))
    }
}

#[async_trait]
impl PeekMessages for PeekNotmuchMessages {
    async fn peek_messages(&self, folder: &str, id: &Id) -> crate::Result<Messages> {
        info!("peeking notmuch messages {id} from folder {folder}");

        let ctx = self.ctx.lock().await;
        let db = ctx.open_db()?;

        let msgs: Messages = id
            .iter()
            .map(|ids| {
                let path = db
                    .find_message(ids)?
                    .ok_or_else(|| {
                        Error::FindEnvelopeEmptyNotmuchError(folder.to_owned(), ids.to_owned())
                    })?
                    .filename()
                    .to_owned();
                let msg = fs::read(path)?;
                Ok(msg)
            })
            .collect::<crate::Result<Vec<_>>>()?
            .into();

        db.close()?;

        Ok(msgs)
    }
}
