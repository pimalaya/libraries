use mail_parser::Message;
use std::{
    io,
    ops::{Deref, DerefMut},
    result,
};
use thiserror::Error;

use crate::mml::{self, CompilerBuilder, InterpreterBuilder, ShowHeadersStrategy};

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot build message from template")]
    CreateMessageBuilderError,
    #[error("cannot compile template")]
    WriteTplToStringError(#[source] io::Error),
    #[error("cannot compile template")]
    WriteTplToVecError(#[source] io::Error),
    #[error("cannot compile mime meta language")]
    CompileMmlError(#[source] mml::compiler::Error),
    #[error("cannot interpret email as a template")]
    InterpretError(#[source] mml::interpreter::Error),
    #[error("cannot parse template")]
    ParseMessageError,
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tpl(String);

impl Deref for Tpl {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Tpl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<String> for Tpl {
    fn from(tpl: String) -> Self {
        Self(tpl)
    }
}

impl Tpl {
    pub fn interpret<B>(bytes: B) -> Result<Self>
    where
        B: AsRef<[u8]>,
    {
        Self::interpret_with(Default::default(), bytes)
    }

    pub fn interpret_with<B>(interpreter: InterpreterBuilder, bytes: B) -> Result<Self>
    where
        B: AsRef<[u8]>,
    {
        let interpreter = interpreter.build();
        let msg = Message::parse(bytes.as_ref()).ok_or(Error::ParseMessageError)?;
        let mut tpl = Self::default();

        for (key, val) in msg.headers_raw() {
            let key = key.trim();
            let val = val.trim();

            match interpreter.show_headers {
                ShowHeadersStrategy::All => tpl.push_str(&format!("{key}: {val}\n")),
                ShowHeadersStrategy::Only(ref keys) if keys.contains(key) => {
                    tpl.push_str(&format!("{key}: {val}\n"))
                }
                ShowHeadersStrategy::Only(_) => (),
            }
        }

        if !tpl.is_empty() {
            tpl.push_str("\n");
        }

        tpl.push_str(&interpreter.interpret(&msg).map_err(Error::InterpretError)?);

        Ok(Tpl::from(tpl))
    }

    pub fn compile(self) -> Result<Vec<u8>> {
        self.compile_with(Default::default())
    }

    pub fn compile_with(self, compiler: CompilerBuilder) -> Result<Vec<u8>> {
        let tpl = Message::parse(self.as_bytes()).ok_or(Error::ParseMessageError)?;

        let mml = tpl
            .text_bodies()
            .into_iter()
            .filter_map(|part| part.text_contents())
            .fold(String::new(), |mut contents, content| {
                if !contents.is_empty() {
                    contents.push_str("\n\n");
                }
                contents.push_str(content.trim());
                contents
            });

        let mut msg_builder = compiler
            .build()
            .compile(&mml)
            .map_err(Error::CompileMmlError)?;

        for (key, val) in tpl.headers_raw() {
            let key = key.trim().to_owned();
            let val = val.trim().to_owned();
            msg_builder = msg_builder.header(key, mail_builder::headers::raw::Raw::new(val));
        }

        let bytes = msg_builder
            .write_to_vec()
            .map_err(Error::WriteTplToVecError)?;

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use concat_with::concat_line;

    use crate::{InterpreterBuilder, Tpl};

    #[test]
    fn interpret_all_headers() {
        let interpreter = InterpreterBuilder::new().show_all_headers();
        let raw = concat_line!(
            "From: from@localhost",
            "To: to@localhost",
            "Subject: subject",
            "",
            "Hello, world!",
        );
        let tpl = Tpl::interpret_with(interpreter, raw.as_bytes()).unwrap();

        assert_eq!(
            *tpl,
            "From: from@localhost\nTo: to@localhost\nSubject: subject\n\nHello, world!"
        );
    }

    #[test]
    fn interpret_only_headers() {
        let interpreter = InterpreterBuilder::new().show_headers(["From", "Subject"]);
        let raw = concat_line!(
            "From: from@localhost",
            "To: to@localhost",
            "Subject: subject",
            "",
            "Hello, world!",
        );
        let tpl = Tpl::interpret_with(interpreter, raw.as_bytes()).unwrap();

        assert_eq!(
            *tpl,
            "From: from@localhost\nSubject: subject\n\nHello, world!"
        );
    }

    #[test]
    fn interpret_no_headers() {
        let interpreter = InterpreterBuilder::new().hide_all_headers();
        let raw = concat_line!(
            "From: from@localhost",
            "To: to@localhost",
            "Subject: subject",
            "",
            "Hello, world!",
        );
        let tpl = Tpl::interpret_with(interpreter, raw.as_bytes()).unwrap();

        assert_eq!(*tpl, "Hello, world!");
    }
}