//! EnvRelay Core.
//!
//! The skill is the brain: it decides what to back up, copies it into a staging
//! directory and writes the manifest. This binary does the one thing the agent
//! must not do — hold the user's passphrase — and nothing else.
//!
//! A `.envrelay` file is an age(scrypt) encrypted `tar + zstd` stream, so the
//! escape hatch is a promise, not an accident:
//!
//! ```text
//! age -d backup.envrelay | zstd -d | tar -xp
//! ```

mod backup;
mod passphrase;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "envrelay",
    version,
    about = "Encrypt and decrypt a development-environment backup with a passphrase.",
    long_about = "Encrypt and decrypt a development-environment backup with a passphrase.\n\n\
                  A .envrelay file is an age(scrypt) encrypted tar + zstd stream. Even without \
                  this binary a backup can always be opened with standard tools:\n\n    \
                  age -d backup.envrelay | zstd -d | tar -xp\n\n\
                  There is no key escrow and no recovery path: lose the passphrase and the \
                  backup is gone."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pack a staging directory into one encrypted backup file.
    Encrypt {
        /// Staging directory to pack. Must exist and must not be empty.
        staging_dir: PathBuf,

        /// Backup file to write. Must not already exist.
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,

        #[command(flatten)]
        passphrase: PassphraseSource,
    },

    /// Unpack an encrypted backup file into a directory.
    Decrypt {
        /// Backup file to open.
        backup_file: PathBuf,

        /// Directory to restore into. Must not exist, or must be empty.
        #[arg(short, long, value_name = "DIR")]
        output: PathBuf,

        #[command(flatten)]
        passphrase: PassphraseSource,
    },

    /// List a backup's contents and print its manifest, writing nothing to disk.
    Inspect {
        /// Backup file to look inside.
        backup_file: PathBuf,

        #[command(flatten)]
        passphrase: PassphraseSource,
    },

    /// Read a backup end to end to prove the passphrase and the file are good, writing nothing to disk.
    Verify {
        /// Backup file to check.
        backup_file: PathBuf,

        #[command(flatten)]
        passphrase: PassphraseSource,
    },
}

#[derive(Args)]
struct PassphraseSource {
    /// Read the passphrase from the first line of FILE.
    ///
    /// For automated tests only. A passphrase in a file is a passphrase on
    /// disk; real backups are encrypted by typing it at the terminal prompt.
    #[arg(long, value_name = "FILE")]
    passphrase_file: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match &cli.command {
        Command::Encrypt {
            staging_dir,
            output,
            passphrase,
        } => backup::encrypt(staging_dir, output, passphrase.passphrase_file.as_deref()),
        Command::Decrypt {
            backup_file,
            output,
            passphrase,
        } => backup::decrypt(backup_file, output, passphrase.passphrase_file.as_deref()),
        Command::Inspect {
            backup_file,
            passphrase,
        } => backup::inspect(backup_file, passphrase.passphrase_file.as_deref()),
        Command::Verify {
            backup_file,
            passphrase,
        } => backup::verify(backup_file, passphrase.passphrase_file.as_deref()),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("envrelay: {error:#}");
            ExitCode::FAILURE
        }
    }
}
