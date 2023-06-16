#[cfg(feature = "imap-backend")]
pub mod imap;
pub mod maildir;
pub mod sync;

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    ops, result,
    str::FromStr,
};
use thiserror::Error;

pub use self::sync::sync_all;

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot parse unknown flag {0}")]
    ParseFlagError(String),
}

type Result<T> = result::Result<T, Error>;

/// Represents the list of flags.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Flags(pub HashSet<Flag>);

impl Hash for Flags {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut flags = Vec::from_iter(self.iter());
        flags.sort_by(|a, b| a.partial_cmp(b).unwrap());
        flags.hash(state)
    }
}

impl ToString for Flags {
    fn to_string(&self) -> String {
        self.iter().fold(String::new(), |mut flags, flag| {
            if !flags.is_empty() {
                flags.push(' ')
            }
            flags.push_str(&flag.to_string());
            flags
        })
    }
}

impl ops::Deref for Flags {
    type Target = HashSet<Flag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for Flags {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<&str> for Flags {
    fn from(s: &str) -> Self {
        s.split_whitespace().flat_map(|flag| flag.parse()).collect()
    }
}

impl From<String> for Flags {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl FromStr for Flags {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Flags(
            s.split_whitespace()
                .map(|flag| flag.parse())
                .collect::<Result<HashSet<_>>>()?,
        ))
    }
}

impl FromIterator<Flag> for Flags {
    fn from_iter<T: IntoIterator<Item = Flag>>(iter: T) -> Self {
        let mut flags = Flags::default();
        flags.extend(iter);
        flags
    }
}

impl Into<Vec<String>> for Flags {
    fn into(self) -> Vec<String> {
        self.iter().map(|flag| flag.to_string()).collect()
    }
}

/// Represents the flag variants.
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd)]
pub enum Flag {
    Seen,
    Answered,
    Flagged,
    Deleted,
    Draft,
    Custom(String),
}

impl Flag {
    pub fn custom<F>(flag: F) -> Self
    where
        F: ToString,
    {
        Self::Custom(flag.to_string())
    }
}

impl From<&str> for Flag {
    fn from(s: &str) -> Self {
        match s.trim() {
            seen if seen.eq_ignore_ascii_case("seen") => Flag::Seen,
            answered if answered.eq_ignore_ascii_case("answered") => Flag::Answered,
            replied if replied.eq_ignore_ascii_case("replied") => Flag::Answered,
            flagged if flagged.eq_ignore_ascii_case("flagged") => Flag::Flagged,
            deleted if deleted.eq_ignore_ascii_case("deleted") => Flag::Deleted,
            trashed if trashed.eq_ignore_ascii_case("trashed") => Flag::Deleted,
            draft if draft.eq_ignore_ascii_case("draft") => Flag::Draft,
            flag => Flag::Custom(flag.into()),
        }
    }
}

impl FromStr for Flag {
    type Err = Error;

    fn from_str(slice: &str) -> Result<Self> {
        match slice.trim() {
            seen if seen.eq_ignore_ascii_case("seen") => Ok(Flag::Seen),
            answered if answered.eq_ignore_ascii_case("answered") => Ok(Flag::Answered),
            replied if replied.eq_ignore_ascii_case("replied") => Ok(Flag::Answered),
            flagged if flagged.eq_ignore_ascii_case("flagged") => Ok(Flag::Flagged),
            deleted if deleted.eq_ignore_ascii_case("deleted") => Ok(Flag::Deleted),
            trashed if trashed.eq_ignore_ascii_case("trashed") => Ok(Flag::Deleted),
            draft if draft.eq_ignore_ascii_case("draft") => Ok(Flag::Draft),
            unknown => Err(Error::ParseFlagError(unknown.to_string())),
        }
    }
}

impl TryFrom<String> for Flag {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl ToString for Flag {
    fn to_string(&self) -> String {
        match self {
            Flag::Seen => "seen".into(),
            Flag::Answered => "answered".into(),
            Flag::Flagged => "flagged".into(),
            Flag::Deleted => "deleted".into(),
            Flag::Draft => "draft".into(),
            Flag::Custom(flag) => flag.clone(),
        }
    }
}