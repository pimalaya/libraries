pub mod config;
pub mod error;

use async_trait::async_trait;
use log::info;
use maildirpp::Maildir;
use shellexpand_utils::{shellexpand_path, try_shellexpand_path};
use std::{ops::Deref, sync::Arc};
use tokio::sync::Mutex;

use crate::{
    account::config::AccountConfig,
    backend::{
        context::{BackendContext, BackendContextBuilder},
        feature::{BackendFeature, CheckUp},
    },
    envelope::{
        get::{maildir::GetMaildirEnvelope, GetEnvelope},
        list::{maildir::ListMaildirEnvelopes, ListEnvelopes},
        watch::{maildir::WatchMaildirEnvelopes, WatchEnvelopes},
    },
    flag::{
        add::{maildir::AddMaildirFlags, AddFlags},
        remove::{maildir::RemoveMaildirFlags, RemoveFlags},
        set::{maildir::SetMaildirFlags, SetFlags},
    },
    folder::{
        add::{maildir::AddMaildirFolder, AddFolder},
        delete::{maildir::DeleteMaildirFolder, DeleteFolder},
        expunge::{maildir::ExpungeMaildirFolder, ExpungeFolder},
        list::{maildir::ListMaildirFolders, ListFolders},
        FolderKind,
    },
    maildir,
    message::{
        add::{maildir::AddMaildirMessage, AddMessage},
        copy::{maildir::CopyMaildirMessages, CopyMessages},
        delete::{maildir::DeleteMaildirMessages, DeleteMessages},
        get::{maildir::GetMaildirMessages, GetMessages},
        peek::{maildir::PeekMaildirMessages, PeekMessages},
        r#move::{maildir::MoveMaildirMessages, MoveMessages},
        remove::{maildir::RemoveMaildirMessages, RemoveMessages},
    },
};

use self::config::MaildirConfig;

/// The Maildir backend context.
///
/// This context is unsync, which means it cannot be shared between
/// threads. For the sync version, see [`MaildirContextSync`].
pub struct MaildirContext {
    /// The account configuration.
    pub account_config: Arc<AccountConfig>,

    /// The Maildir configuration.
    pub maildir_config: Arc<MaildirConfig>,

    /// The maildir instance.
    pub root: Maildir,
}

impl MaildirContext {
    /// Create a maildir instance from a folder name.
    pub fn get_maildir_from_folder_name(&self, folder: &str) -> Result<Maildir, error::Error> {
        // If the folder matches to the inbox folder kind, create a
        // maildir instance from the root folder.
        if FolderKind::matches_inbox(folder) {
            return try_shellexpand_path(self.root.path())
                .map(Maildir::from)
                .map_err(Into::into);
        }

        let folder = self.account_config.get_folder_alias(folder);

        // If the folder is a valid maildir path, create a maildir
        // instance from it. First check for absolute path…
        try_shellexpand_path(&folder)
            // then check for relative path to `maildir-dir`…
            .or_else(|_| try_shellexpand_path(self.root.path().join(&folder)))
            // TODO: should move to CLI
            // // and finally check for relative path to the current
            // // directory
            // .or_else(|_| {
            //     try_shellexpand_path(
            //         env::current_dir()
            //             .map_err(Error::GetCurrentFolderError)?
            //             .join(&folder),
            //     )
            // })
            .or_else(|_| {
                // Otherwise create a maildir instance from a maildir
                // subdirectory by adding a "." in front of the name
                // as described in the [spec].
                //
                // [spec]: http://www.courier-mta.org/imap/README.maildirquota.html
                let folder = maildir::encode_folder(&folder);
                try_shellexpand_path(self.root.path().join(format!(".{}", folder)))
            })
            .map(Maildir::from)
            .map_err(Into::into)
    }
}

/// The sync version of the Maildir backend context.
///
/// This is just a Maildir session wrapped into a mutex, so the same
/// Maildir session can be shared and updated across multiple threads.
#[derive(Clone)]
pub struct MaildirContextSync {
    /// The account configuration.
    pub account_config: Arc<AccountConfig>,

    /// The Maildir configuration.
    pub maildir_config: Arc<MaildirConfig>,

    inner: Arc<Mutex<MaildirContext>>,
}

impl Deref for MaildirContextSync {
    type Target = Arc<Mutex<MaildirContext>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl BackendContext for MaildirContextSync {}

/// The Maildir backend context builder.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MaildirContextBuilder {
    /// The account configuration.
    pub account_config: Arc<AccountConfig>,

    /// The Maildir configuration.
    pub mdir_config: Arc<MaildirConfig>,
}

impl MaildirContextBuilder {
    pub fn new(account_config: Arc<AccountConfig>, mdir_config: Arc<MaildirConfig>) -> Self {
        Self {
            account_config,
            mdir_config,
        }
    }
}

#[async_trait]
impl BackendContextBuilder for MaildirContextBuilder {
    type Context = MaildirContextSync;

    fn check_up(&self) -> Option<BackendFeature<Self::Context, dyn CheckUp>> {
        Some(Arc::new(CheckUpMaildir::some_new_boxed))
    }

    fn add_folder(&self) -> Option<BackendFeature<Self::Context, dyn AddFolder>> {
        Some(Arc::new(AddMaildirFolder::some_new_boxed))
    }

    fn list_folders(&self) -> Option<BackendFeature<Self::Context, dyn ListFolders>> {
        Some(Arc::new(ListMaildirFolders::some_new_boxed))
    }

    fn expunge_folder(&self) -> Option<BackendFeature<Self::Context, dyn ExpungeFolder>> {
        Some(Arc::new(ExpungeMaildirFolder::some_new_boxed))
    }

    // TODO
    // fn purge_folder(&self) -> Option<BackendFeature<Self::Context, dyn PurgeFolder>> {
    //     Some(Arc::new(PurgeMaildirFolder::some_new_boxed))
    // }

    fn delete_folder(&self) -> Option<BackendFeature<Self::Context, dyn DeleteFolder>> {
        Some(Arc::new(DeleteMaildirFolder::some_new_boxed))
    }

    fn get_envelope(&self) -> Option<BackendFeature<Self::Context, dyn GetEnvelope>> {
        Some(Arc::new(GetMaildirEnvelope::some_new_boxed))
    }

    fn list_envelopes(&self) -> Option<BackendFeature<Self::Context, dyn ListEnvelopes>> {
        Some(Arc::new(ListMaildirEnvelopes::some_new_boxed))
    }

    fn watch_envelopes(&self) -> Option<BackendFeature<Self::Context, dyn WatchEnvelopes>> {
        Some(Arc::new(WatchMaildirEnvelopes::some_new_boxed))
    }

    fn add_flags(&self) -> Option<BackendFeature<Self::Context, dyn AddFlags>> {
        Some(Arc::new(AddMaildirFlags::some_new_boxed))
    }

    fn set_flags(&self) -> Option<BackendFeature<Self::Context, dyn SetFlags>> {
        Some(Arc::new(SetMaildirFlags::some_new_boxed))
    }

    fn remove_flags(&self) -> Option<BackendFeature<Self::Context, dyn RemoveFlags>> {
        Some(Arc::new(RemoveMaildirFlags::some_new_boxed))
    }

    fn add_message(&self) -> Option<BackendFeature<Self::Context, dyn AddMessage>> {
        Some(Arc::new(AddMaildirMessage::some_new_boxed))
    }

    fn peek_messages(&self) -> Option<BackendFeature<Self::Context, dyn PeekMessages>> {
        Some(Arc::new(PeekMaildirMessages::some_new_boxed))
    }

    fn get_messages(&self) -> Option<BackendFeature<Self::Context, dyn GetMessages>> {
        Some(Arc::new(GetMaildirMessages::some_new_boxed))
    }

    fn copy_messages(&self) -> Option<BackendFeature<Self::Context, dyn CopyMessages>> {
        Some(Arc::new(CopyMaildirMessages::some_new_boxed))
    }

    fn move_messages(&self) -> Option<BackendFeature<Self::Context, dyn MoveMessages>> {
        Some(Arc::new(MoveMaildirMessages::some_new_boxed))
    }

    fn delete_messages(&self) -> Option<BackendFeature<Self::Context, dyn DeleteMessages>> {
        Some(Arc::new(DeleteMaildirMessages::some_new_boxed))
    }

    fn remove_messages(&self) -> Option<BackendFeature<Self::Context, dyn RemoveMessages>> {
        Some(Arc::new(RemoveMaildirMessages::some_new_boxed))
    }

    async fn build(self) -> crate::Result<Self::Context> {
        info!("building new maildir context");

        let path = shellexpand_path(&self.mdir_config.root_dir);

        let root = Maildir::from(path);
        root.create_dirs()?;

        let ctx = MaildirContext {
            account_config: self.account_config.clone(),
            maildir_config: self.mdir_config.clone(),
            root,
        };

        Ok(MaildirContextSync {
            account_config: self.account_config,
            maildir_config: self.mdir_config,
            inner: Arc::new(Mutex::new(ctx)),
        })
    }
}

#[derive(Clone)]
pub struct CheckUpMaildir {
    pub ctx: MaildirContextSync,
}

impl CheckUpMaildir {
    pub fn new(ctx: &MaildirContextSync) -> Self {
        Self { ctx: ctx.clone() }
    }

    pub fn new_boxed(ctx: &MaildirContextSync) -> Box<dyn CheckUp> {
        Box::new(Self::new(ctx))
    }

    pub fn some_new_boxed(ctx: &MaildirContextSync) -> Option<Box<dyn CheckUp>> {
        Some(Self::new_boxed(ctx))
    }
}

#[async_trait]
impl CheckUp for CheckUpMaildir {
    async fn check_up(&self) -> crate::Result<()> {
        let ctx = self.ctx.lock().await;

        ctx.root
            .list_cur()
            .try_for_each(|e| e.map(|_| ()))
            .map_err(error::Error::CheckingUpMaildirFailed)?;

        Ok(())
    }
}

/// URL-encode the given folder.
pub fn encode_folder(folder: impl AsRef<str>) -> String {
    urlencoding::encode(folder.as_ref()).to_string()
}

/// URL-decode the given folder.
pub fn decode_folder(folder: impl AsRef<str> + ToString) -> String {
    urlencoding::decode(folder.as_ref())
        .map(|folder| folder.to_string())
        .unwrap_or_else(|_| folder.to_string())
}
