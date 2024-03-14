use mml::{
    pgp::{CmdsPgp, Pgp},
    MmlCompilerBuilder,
};
use process::Command;

#[tokio::main]
async fn main() {
    env_logger::builder().is_test(true).init();

    let mml = include_str!("./pgp.eml");
    let mml_compiler = MmlCompilerBuilder::new()
        .with_pgp(Pgp::Cmds(CmdsPgp {
            encrypt_cmd: Some(Command::from(
                "gpg --homedir ./tests/gpg-home -eqa <recipients>",
            )),
            encrypt_recipient_fmt: Some(CmdsPgp::default_encrypt_recipient_fmt()),
            encrypt_recipients_sep: Some(CmdsPgp::default_encrypt_recipients_sep()),
            decrypt_cmd: Some(Command::from("gpg --homedir ./tests/gpg-home -dq")),
            sign_cmd: Some(Command::from("gpg --homedir ./tests/gpg-home -saq")),
            verify_cmd: Some(Command::from("gpg --homedir ./tests/gpg-home --verify -q")),
        }))
        .build(mml)
        .unwrap();
    let mime = mml_compiler.compile().await.unwrap().into_string().unwrap();

    println!("================================");
    println!("MML MESSAGE");
    println!("================================");
    println!();
    println!("{mml}");

    println!("================================");
    println!("COMPILED MIME MESSAGE");
    println!("================================");
    println!();
    println!("{mime}");
}
