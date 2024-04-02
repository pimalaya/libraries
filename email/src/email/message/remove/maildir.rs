use async_trait::async_trait;
use log::info;

use crate::{email::error::Error, envelope::Id, maildir::MaildirContextSync};

use super::RemoveMessages;

#[derive(Clone)]
pub struct RemoveMaildirMessages {
    ctx: MaildirContextSync,
}

impl RemoveMaildirMessages {
    pub fn new(ctx: &MaildirContextSync) -> Self {
        Self { ctx: ctx.clone() }
    }

    pub fn new_boxed(ctx: &MaildirContextSync) -> Box<dyn RemoveMessages> {
        Box::new(Self::new(ctx))
    }

    pub fn some_new_boxed(ctx: &MaildirContextSync) -> Option<Box<dyn RemoveMessages>> {
        Some(Self::new_boxed(ctx))
    }
}

#[async_trait]
impl RemoveMessages for RemoveMaildirMessages {
    async fn remove_messages(&self, folder: &str, id: &Id) -> crate::Result<()> {
        info!("removing maildir message(s) {id} from folder {folder}");

        let ctx = self.ctx.lock().await;
        let mdir = ctx.get_maildir_from_folder_name(folder)?;

        id.iter().try_for_each(|ref id| {
            mdir.delete(id).map_err(|err| {
                Error::RemoveMaildirMessageError(err, folder.to_owned(), id.to_string())
            })
        })?;

        Ok(())
    }
}
