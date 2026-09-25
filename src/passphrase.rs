//! Where the passphrase comes from.
//!
//! The terminal, and — for automated tests — a file. Deliberately not:
//! `--passphrase`, an environment variable, or stdin. Those three leak into
//! shell history, `ps` output and the transcript of whatever agent is driving
//! the session, which is exactly what this binary exists to prevent.

use std::fs;
use std::path::Path;

use age::secrecy::SecretString;
use anyhow::{Context, Result, bail};

/// The passphrase for a new backup: typed twice, and the two must match.
pub fn for_encrypt(passphrase_file: Option<&Path>) -> Result<SecretString> {
    if let Some(path) = passphrase_file {
        return from_file(path);
    }

    let first = prompt("Passphrase: ")?;
    let second = prompt("Confirm passphrase: ")?;
    if first != second {
        bail!("the two passphrases do not match");
    }
    accept(first)
}

/// The passphrase for an existing backup: typed once.
pub fn for_decrypt(passphrase_file: Option<&Path>) -> Result<SecretString> {
    match passphrase_file {
        Some(path) => from_file(path),
        None => accept(prompt("Passphrase: ")?),
    }
}

/// Reads from `/dev/tty` rather than stdin, so a redirected or piped stdin
/// cannot stand in for a person at a keyboard.
fn prompt(label: &str) -> Result<String> {
    rpassword::prompt_password(label).context(
        "could not read the passphrase from the terminal; \
         run this command yourself in a terminal window",
    )
}

fn from_file(path: &Path) -> Result<SecretString> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("could not read passphrase file {}", path.display()))?;
    accept(contents.lines().next().unwrap_or_default().to_owned())
}

fn accept(passphrase: String) -> Result<SecretString> {
    if passphrase.is_empty() {
        bail!("the passphrase is empty; an empty passphrase protects nothing");
    }
    Ok(SecretString::from(passphrase))
}
