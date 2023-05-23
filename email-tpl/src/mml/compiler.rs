use log::warn;
use mail_builder::{mime::MimePart, MessageBuilder};
use pimalaya_process::Cmd;
use std::{borrow::Cow, env, ffi::OsStr, fs, io, path::PathBuf, result};
use thiserror::Error;

use crate::mml::parsers::{self, prelude::*};

use super::tokens::{Part, DISPOSITION, ENCRYPT, FILENAME, NAME, SIGN, TYPE};

#[derive(Debug, Error)]
pub enum Error {
    // TODO: return the original chumsky::Error
    #[error("cannot parse template: {0}")]
    ParseTplError(String),
    #[error("cannot compile template: recipient is missing")]
    CompileTplMissingRecipientError,
    #[error("cannot compile template")]
    WriteCompiledPartToVecError(#[source] io::Error),
    #[error("cannot find missing property filename")]
    GetFilenamePropMissingError,
    #[error("cannot expand filename {1}")]
    ExpandFilenameError(#[source] shellexpand::LookupError<env::VarError>, String),
    #[error("cannot read attachment at {1}")]
    ReadAttachmentError(#[source] io::Error, String),
    #[error("cannot encrypt multi part")]
    EncryptPartError(#[from] pimalaya_process::Error),
    #[error("cannot sign multi part")]
    SignPartError(#[source] pimalaya_process::Error),
}

pub type Result<T> = result::Result<T, Error>;

/// Represents the compiler builder. It allows you to customize the
/// template compilation using the [Builder pattern].
///
/// [Builder pattern]: https://en.wikipedia.org/wiki/Builder_pattern
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilerBuilder {
    /// Represents the PGP encrypt system command. Defaults to `gpg
    /// --encrypt --armor --recipient <recipient> --quiet --output -`.
    pgp_encrypt_cmd: Option<Cmd>,

    /// Represents the PGP encrypt recipient. By default, it will take
    /// the first address found from the "To" header of the template
    /// being compiled.
    pgp_encrypt_recipient: Option<String>,

    /// Represents the PGP sign system command. Defaults to `gpg
    /// --sign --armor --quiet --output -`.
    pgp_sign_cmd: Option<Cmd>,
}

impl<'a> CompilerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pgp_encrypt_cmd<C: Into<Cmd>>(mut self, cmd: C) -> Self {
        self.pgp_encrypt_cmd = Some(cmd.into());
        self
    }

    pub fn some_pgp_encrypt_cmd<C: Into<Cmd>>(mut self, cmd: Option<C>) -> Self {
        self.pgp_encrypt_cmd = cmd.map(|c| c.into());
        self
    }

    pub fn pgp_encrypt_recipient<R: AsRef<str>>(mut self, recipient: R) -> Self {
        match recipient.as_ref().parse() {
            Ok(mbox) => {
                self.pgp_encrypt_recipient = Some(mbox);
            }
            Err(err) => {
                warn!(
                    "skipping invalid pgp encrypt recipient {}: {}",
                    recipient.as_ref(),
                    err
                );
            }
        }
        self
    }

    pub fn pgp_sign_cmd<C: Into<Cmd>>(mut self, cmd: C) -> Self {
        self.pgp_sign_cmd = Some(cmd.into());
        self
    }

    pub fn some_pgp_sign_cmd<C: Into<Cmd>>(mut self, cmd: Option<C>) -> Self {
        self.pgp_sign_cmd = cmd.map(|c| c.into());
        self
    }

    pub fn build(self) -> Compiler {
        Compiler {
            pgp_encrypt_cmd: self.pgp_encrypt_cmd.unwrap_or_else(|| {
                "gpg --encrypt --armor --recipient <recipient> --quiet --output -".into()
            }),
            pgp_encrypt_recipient: self.pgp_encrypt_recipient,
            pgp_sign_cmd: self
                .pgp_sign_cmd
                .unwrap_or_else(|| "gpg --sign --armor --quiet --output -".into()),
        }
    }
}

/// Represents the compiler options. It is the final struct passed
/// down to the [Tpl::compile] function.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Compiler {
    pub pgp_encrypt_cmd: Cmd,
    pub pgp_encrypt_recipient: Option<String>,
    pub pgp_sign_cmd: Cmd,
}

impl<'a> Compiler {
    /// Compiles the given string template into a raw MIME Message
    /// using [CompilerOpts] from the builder.
    pub fn compile<T: AsRef<str>>(&self, tpl: T) -> Result<MessageBuilder<'a>> {
        let parts = parsers::parts()
            .parse(tpl.as_ref())
            .map_err(|errs| Error::ParseTplError(errs[0].to_string()))?;
        self.compile_parts(parts)
    }

    /// Builds the final PGP encrypt system command by replacing
    /// `<recipient>` occurrences with the actual recipient. Fails in
    /// case no recipient is found.
    fn pgp_encrypt_cmd(&self) -> Result<Cmd> {
        let recipient = self
            .pgp_encrypt_recipient
            .as_ref()
            .ok_or(Error::CompileTplMissingRecipientError)?;

        let cmd = self
            .pgp_encrypt_cmd
            .clone()
            .replace("<recipient>", &recipient.to_string());

        Ok(cmd)
    }

    fn compile_parts<P>(&self, parts: P) -> Result<MessageBuilder<'a>>
    where
        P: IntoIterator<Item = Part>,
    {
        let parts: Vec<Part> = parts.into_iter().collect();

        let mut builder = MessageBuilder::new();

        builder = match parts.len() {
            0 => builder.text_body(String::new()),
            1 => builder.body(self.compile_part(parts.into_iter().next().unwrap())?),
            _ => builder.body(MimePart::new_multipart(
                "multipart/mixed",
                parts
                    .into_iter()
                    .map(|part| self.compile_part(part))
                    .collect::<Result<Vec<_>>>()?,
            )),
        };

        Ok(builder)
    }

    fn compile_part(&self, part: Part) -> Result<MimePart<'a>> {
        match part {
            Part::MultiPart((props, parts)) => {
                let mut multi_part = match props.get(TYPE).map(String::as_str) {
                    Some("mixed") | None => MimePart::new_multipart("multipart/mixed", Vec::new()),
                    Some("alternative") => {
                        MimePart::new_multipart("multipart/alternative", Vec::new())
                    }
                    Some("related") => MimePart::new_multipart("multipart/related", Vec::new()),
                    Some(unknown) => {
                        warn!("unknown multipart type {unknown}, falling back to mixed");
                        MimePart::new_multipart("multipart/mixed", Vec::new())
                    }
                };

                for part in Part::compact_text_plain_parts(parts) {
                    multi_part.add_part(self.compile_part(part)?)
                }

                let multi_part = match props.get(SIGN).map(String::as_str) {
                    Some("command") => {
                        let mut buf = Vec::new();
                        multi_part
                            .write_part(&mut buf)
                            .map_err(Error::WriteCompiledPartToVecError)?;
                        Part::sign(buf, self.pgp_sign_cmd.clone()).map_err(Error::SignPartError)
                    }
                    _ => Ok(multi_part),
                }?;

                let multi_part = match props.get(ENCRYPT).map(String::as_str) {
                    Some("command") => {
                        let mut buf = Vec::new();
                        multi_part
                            .write_part(&mut buf)
                            .map_err(Error::WriteCompiledPartToVecError)?;
                        Part::encrypt(buf, self.pgp_encrypt_cmd()?).map_err(Error::EncryptPartError)
                    }
                    _ => Ok(multi_part),
                }?;

                Ok(multi_part)
            }
            Part::SinglePart((ref props, body)) => {
                let ctype = Part::get_or_guess_content_type(props, &body);
                let mut part = MimePart::new_binary(ctype, Cow::Owned(body.into_bytes()));

                part = match props.get(DISPOSITION).map(String::as_str) {
                    Some("inline") => part.inline(),
                    Some("attachment") => {
                        let fname = props
                            .get(NAME)
                            .map(ToOwned::to_owned)
                            .unwrap_or("noname".into());
                        part.attachment(fname)
                    }
                    _ => part,
                };

                part = match props.get(SIGN).map(String::as_str) {
                    Some("command") => {
                        let mut buf = Vec::new();
                        part.write_part(&mut buf)
                            .map_err(Error::WriteCompiledPartToVecError)?;
                        Part::sign(buf, self.pgp_sign_cmd.clone()).map_err(Error::SignPartError)
                    }
                    _ => Ok(part),
                }?;

                part = match props.get(ENCRYPT).map(String::as_str) {
                    Some("command") => {
                        let mut buf = Vec::new();
                        part.write_part(&mut buf)
                            .map_err(Error::WriteCompiledPartToVecError)?;
                        Part::encrypt(buf, self.pgp_encrypt_cmd()?).map_err(Error::EncryptPartError)
                    }
                    _ => Ok(part),
                }?;

                Ok(part)
            }
            Part::Attachment(ref props) => {
                let filepath = props
                    .get(FILENAME)
                    .ok_or(Error::GetFilenamePropMissingError)?;
                let filepath = shellexpand::full(&filepath)
                    .map_err(|err| Error::ExpandFilenameError(err, filepath.to_string()))?
                    .to_string();

                let body = fs::read(&filepath)
                    .map_err(|err| Error::ReadAttachmentError(err, filepath.clone()))?;

                let fname = props
                    .get(NAME)
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        PathBuf::from(filepath)
                            .file_name()
                            .and_then(OsStr::to_str)
                            .map(ToOwned::to_owned)
                    })
                    .unwrap_or("noname".into());

                let disposition = props.get(DISPOSITION).map(String::as_str);
                let content_type = Part::get_or_guess_content_type(props, &body);

                let mut part = MimePart::new_binary(content_type, body);

                part = match disposition {
                    Some("inline") => part.inline(),
                    _ => part.attachment(fname),
                };

                part = match props.get(SIGN).map(String::as_str) {
                    Some("command") => {
                        let mut buf = Vec::new();
                        part.write_part(&mut buf)
                            .map_err(Error::WriteCompiledPartToVecError)?;
                        Part::sign(buf, self.pgp_sign_cmd.clone()).map_err(Error::SignPartError)
                    }
                    _ => Ok(part),
                }?;

                part = match props.get(ENCRYPT).map(String::as_str) {
                    Some("command") => {
                        let mut buf = Vec::new();
                        part.write_part(&mut buf)
                            .map_err(Error::WriteCompiledPartToVecError)?;
                        Part::encrypt(buf, self.pgp_encrypt_cmd()?).map_err(Error::EncryptPartError)
                    }
                    _ => Ok(part),
                }?;

                Ok(part)
            }
            Part::TextPlainPart(body) => Ok(MimePart::new_text(body)),
        }
    }
}

#[cfg(test)]
mod tests {
    use concat_with::concat_line;
    use std::{collections::HashMap, io::prelude::*};
    use tempfile::NamedTempFile;

    use crate::mml::{
        compiler::Compiler,
        parsers::{self, prelude::*},
        tokens::Part,
    };

    #[test]
    fn attachment() {
        let mut attachment = NamedTempFile::new().unwrap();
        write!(attachment, "body").unwrap();

        let part = parsers::attachment()
            .parse(format!(
                "<#part name=custom filename={} type=application/octet-stream>",
                attachment.path().to_string_lossy()
            ))
            .unwrap();
        let part = Compiler::default().compile_part(part).unwrap();

        let mut buf = Vec::new();
        part.write_part(&mut buf).unwrap();

        let expected = concat_line!(
            "Content-Type: application/octet-stream\r",
            "Content-Disposition: attachment; filename=\"custom\"\r",
            "Content-Transfer-Encoding: base64\r",
            "\r",
            "Ym9keQ==\r",
            ""
        );

        assert_eq!(String::from_utf8_lossy(&buf), expected);
    }

    #[test]
    fn compact_text_plain_parts() {
        assert_eq!(vec![] as Vec<Part>, Part::compact_text_plain_parts(vec![]));

        assert_eq!(
            vec![Part::TextPlainPart("This is a plain text part.".into())],
            Part::compact_text_plain_parts(vec![Part::TextPlainPart(
                "This is a plain text part.".into()
            )])
        );

        assert_eq!(
            vec![Part::TextPlainPart(
                "This is a plain text part.\n\nThis is a new plain text part.".into()
            )],
            Part::compact_text_plain_parts(vec![
                Part::TextPlainPart("This is a plain text part.".into()),
                Part::TextPlainPart("This is a new plain text part.".into())
            ])
        );

        assert_eq!(
            vec![
                Part::TextPlainPart(
                    "This is a plain text part.\n\nThis is a new plain text part.".into()
                ),
                Part::SinglePart((
                    HashMap::default(),
                    "<h1>This is a HTML text part.</h1>".into()
                ))
            ],
            Part::compact_text_plain_parts(vec![
                Part::TextPlainPart("This is a plain text part.".into()),
                Part::SinglePart((
                    HashMap::default(),
                    "<h1>This is a HTML text part.</h1>".into()
                )),
                Part::TextPlainPart("This is a new plain text part.".into())
            ])
        );
    }
}