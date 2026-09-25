//! Packing a staging directory into an encrypted backup, and back out again.
//!
//! Both directions are fully streaming — `tar -> zstd -> age -> file` on the
//! way in, the reverse on the way out — so the size of a backup is bounded by
//! the disk, not by memory.
//!
//! Neither direction has a partial-success state. If anything fails, whatever
//! was written so far is removed and the process exits non-zero.

use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::iter;
use std::path::{Component, Path};

use age::secrecy::SecretString;
use anyhow::{Context, Result, anyhow, bail};

/// zstd's default level. Compression is never the bottleneck here, and the
/// escape hatch (`zstd -d`) does not care which level produced the stream.
const ZSTD_LEVEL: i32 = 3;

/// Packs `staging_dir` into a new encrypted file at `output`.
pub fn encrypt(staging_dir: &Path, output: &Path, passphrase_file: Option<&Path>) -> Result<()> {
    check_staging_dir(staging_dir)?;
    if output
        .try_exists()
        .with_context(|| format!("could not check {}", output.display()))?
    {
        bail!(
            "{} already exists; envrelay never overwrites a backup, \
             so pick another output path or remove that file first",
            output.display()
        );
    }

    // The output file must sit outside the tree being packed, or tar meets the
    // file it is writing, reads it while it grows, and never reaches its end.
    let staging_resolved = fs::canonicalize(staging_dir)
        .with_context(|| format!("could not resolve {}", staging_dir.display()))?;
    let output_parent = match output.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if let Ok(parent_resolved) = fs::canonicalize(output_parent)
        && parent_resolved.starts_with(&staging_resolved)
    {
        bail!(
            "the output file {} would live inside the staging directory being packed; \
             write the backup somewhere outside {}",
            output.display(),
            staging_dir.display()
        );
    }

    let passphrase = crate::passphrase::for_encrypt(passphrase_file)?;

    // `create_new` rather than a bare `create`: the existence check above is
    // advisory, this is the guarantee. It also happens before the cleanup
    // closure exists, so a file envrelay did not create is never removed.
    // 0600 because the ciphertext regularly carries SSH keys and lands in
    // shared or synced places; age protects the contents, the mode keeps the
    // file itself out of other local users' reach.
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(output)
            .with_context(|| format!("could not create {}", output.display()))?
    };

    pack(file, staging_dir, output, passphrase).inspect_err(|_| {
        // A half-written backup is worse than no backup: it looks like one.
        let _ = fs::remove_file(output);
    })
}

/// Unpacks the encrypted `backup_file` into `output`.
pub fn decrypt(backup_file: &Path, output: &Path, passphrase_file: Option<&Path>) -> Result<()> {
    if !backup_file.is_file() {
        bail!("backup file {} does not exist", backup_file.display());
    }
    // Absolute from here on: `ancestors()` on a relative path bottoms out in
    // an empty path that never "exists", which would break the cleanup below.
    let output = std::path::absolute(output)
        .with_context(|| format!("could not resolve {}", output.display()))?;
    let output = output.as_path();
    // Fail fast, before the user is asked to type a passphrase...
    check_output_dir(output)?;

    let passphrase = crate::passphrase::for_decrypt(passphrase_file)?;

    // ...and look again after: the prompt can sit open for minutes and the
    // path can change underneath it, so this second look is the one the
    // cleanup logic trusts.
    let output_already_existed = check_output_dir(output)?;
    // A pre-existing directory is put back exactly as found on failure — its
    // own mode included, since the rebuild below would otherwise use umask's.
    let original_permissions = if output_already_existed {
        Some(
            fs::metadata(output)
                .with_context(|| format!("could not check {}", output.display()))?
                .permissions(),
        )
    } else {
        None
    };
    // Everything from the first directory created here downward is ours to
    // remove if the restore fails; `None` means `output` was already on disk.
    let cleanup_root = if output_already_existed {
        None
    } else {
        output
            .ancestors()
            .take_while(|ancestor| fs::symlink_metadata(ancestor).is_err())
            .last()
            .map(Path::to_path_buf)
    };

    // 0700 on every directory created here — the plaintext about to land is at
    // least as sensitive as the ciphertext, which is written 0600. A
    // pre-existing empty directory keeps whatever mode its owner chose.
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(output)
            .with_context(|| format!("could not create output directory {}", output.display()))?;
    }

    let Err(error) = unpack(backup_file, output, passphrase).map_err(|error| {
        explain_mid_stream_corruption(error, "nothing from this restore was kept")
    }) else {
        return Ok(());
    };

    // Leave the filesystem as we found it: nothing restored, no directories
    // this run created, and a directory that was already there put back empty.
    let cleaned = match &cleanup_root {
        Some(root) => fs::remove_dir_all(root),
        None => fs::remove_dir_all(output)
            .and_then(|()| fs::create_dir(output))
            .and_then(|()| match &original_permissions {
                Some(permissions) => fs::set_permissions(output, permissions.clone()),
                None => Ok(()),
            }),
    };
    Err(match cleaned {
        Ok(()) => error,
        // Saying "nothing was kept" when something was would send the user
        // away from plaintext still sitting on disk.
        Err(_) => error.context(format!(
            "worse: the partially restored plaintext in {} could not be removed — \
             delete that directory yourself",
            output.display()
        )),
    })
}

/// Prints what is inside `backup_file` — entry listing plus the raw
/// `manifest.json` — without writing anything to disk.
pub fn inspect(backup_file: &Path, passphrase_file: Option<&Path>) -> Result<()> {
    if !backup_file.is_file() {
        bail!("backup file {} does not exist", backup_file.display());
    }
    let passphrase = crate::passphrase::for_decrypt(passphrase_file)?;

    let stream = open_stream(backup_file, passphrase)?;
    let mut archive = tar::Archive::new(stream);

    let mut entries: u64 = 0;
    let mut bytes: u64 = 0;
    let mut manifest: Option<Vec<u8>> = None;
    let mut manifest_too_large = false;
    let result: Result<()> = (|| {
        for entry in archive.entries().context("could not read the archive")? {
            let mut entry = entry.context("could not read the next file in the archive")?;
            let path = entry
                .path()
                .context("an archive entry has an unreadable path")?
                .into_owned();
            entries += 1;
            bytes += entry.size();
            print_entry_line(&entry, &path)?;

            if manifest.is_none() && path_is_manifest(&path) {
                // The manifest is the table of contents and rarely more than
                // a few hundred KB; a multi-megabyte one is printed as a
                // note rather than read into memory.
                if entry.size() <= MANIFEST_PRINT_LIMIT {
                    let mut contents = Vec::new();
                    entry
                        .read_to_end(&mut contents)
                        .context("could not read manifest.json from the archive")?;
                    manifest = Some(contents);
                } else {
                    manifest_too_large = true;
                }
            }
        }
        // Same drain as verify: a truncation confined to the stream's tail
        // must not let inspect print a full listing and exit clean.
        let mut rest = archive.into_inner();
        io::copy(&mut rest, &mut io::sink())
            .context("could not read the end of the backup file")?;
        Ok(())
    })();
    result.map_err(|error| {
        explain_mid_stream_corruption(error, "the listing stops where the damage starts")
    })?;

    println!();
    println!("{entries} entries, {bytes} bytes when restored");
    match manifest {
        Some(contents) => {
            println!();
            println!("--- manifest.json ---");
            println!("{}", String::from_utf8_lossy(&contents).trim_end());
        }
        None if manifest_too_large => {
            println!(
                "note: manifest.json is larger than {MANIFEST_PRINT_LIMIT} bytes; not printing it"
            )
        }
        None => println!("note: no manifest.json at the archive root"),
    }
    Ok(())
}

/// Reads `backup_file` end to end — every entry, through to the final
/// authentication tag — writing nothing. Success means the passphrase is
/// correct and every byte of the file is intact.
pub fn verify(backup_file: &Path, passphrase_file: Option<&Path>) -> Result<()> {
    if !backup_file.is_file() {
        bail!("backup file {} does not exist", backup_file.display());
    }
    let passphrase = crate::passphrase::for_decrypt(passphrase_file)?;

    let stream = open_stream(backup_file, passphrase)?;
    let mut archive = tar::Archive::new(stream);

    let mut entries: u64 = 0;
    let mut bytes: u64 = 0;
    let result: Result<()> = (|| {
        for entry in archive.entries().context("could not read the archive")? {
            let mut entry = entry.context("could not read the next file in the archive")?;
            let path = entry
                .path()
                .context("an archive entry has an unreadable path")?
                .into_owned();
            // An entry that decrypt would refuse should not verify as good.
            check_archive_path(&path)?;
            entries += 1;
            bytes += io::copy(&mut entry, &mut io::sink())
                .with_context(|| format!("could not read {} from the archive", path.display()))?;
        }
        // Drain past the archive's trailer so the last compression frame and
        // the final age STREAM chunk are authenticated too — "every entry
        // read fine" is not yet "every byte of the file is intact".
        let mut rest = archive.into_inner();
        io::copy(&mut rest, &mut io::sink())
            .context("could not read the end of the backup file")?;
        Ok(())
    })();
    result.map_err(|error| {
        explain_mid_stream_corruption(error, "a decrypt of this file would fail the same way")
    })?;

    println!(
        "ok: {entries} entries, {bytes} bytes when restored; \
         the passphrase is correct and the whole file is intact"
    );
    Ok(())
}

/// 8 MiB — far above any real manifest, far below memory that matters.
const MANIFEST_PRINT_LIMIT: u64 = 8 * 1024 * 1024;

fn path_is_manifest(path: &Path) -> bool {
    let mut components = path.components().filter(|c| *c != Component::CurDir);
    components.next() == Some(Component::Normal("manifest.json".as_ref()))
        && components.next().is_none()
}

fn print_entry_line(entry: &tar::Entry<impl Read>, path: &Path) -> Result<()> {
    let header = entry.header();
    let kind = match header.entry_type() {
        tar::EntryType::Directory => 'd',
        tar::EntryType::Symlink => 'l',
        tar::EntryType::Regular => '-',
        _ => '?',
    };
    // The header mode carries file-type bits too (0o100644); the kind
    // character already says what the entry is, so print permissions alone.
    let mode = header
        .mode()
        .context("an archive entry has an unreadable mode")?
        & 0o7777;
    let link = match entry.link_name().ok().flatten() {
        Some(target) => format!(" -> {}", target.display()),
        None => String::new(),
    };
    println!(
        "{kind}{mode:04o} {size:>12} {path}{link}",
        size = entry.size(),
        path = path.display()
    );
    Ok(())
}

/// age's STREAM MAC reports mid-payload corruption as an `InvalidData` io
/// error buried under the unpacker's own wrapping; the person reading the
/// message needs "the file is bad", not that call stack.
fn explain_mid_stream_corruption(error: anyhow::Error, aftermath: &str) -> anyhow::Error {
    let corrupted = error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::InvalidData)
    });
    if corrupted {
        error.context(format!(
            "the backup file is corrupted or truncated; \
             the passphrase may well be right, but the data cannot be trusted \
             and {aftermath} — try another copy of the file",
        ))
    } else {
        error
    }
}

fn check_staging_dir(staging_dir: &Path) -> Result<()> {
    let metadata = fs::metadata(staging_dir).with_context(|| {
        format!(
            "staging directory {} is not readable",
            staging_dir.display()
        )
    })?;
    if !metadata.is_dir() {
        bail!("{} is not a directory", staging_dir.display());
    }
    let is_empty = fs::read_dir(staging_dir)
        .with_context(|| format!("could not read {}", staging_dir.display()))?
        .next()
        .is_none();
    if is_empty {
        bail!(
            "staging directory {} is empty; there is nothing to encrypt",
            staging_dir.display()
        );
    }
    Ok(())
}

/// Returns whether the directory was already on disk, so a failed decrypt can
/// restore that state exactly.
fn check_output_dir(output: &Path) -> Result<bool> {
    // `symlink_metadata`, not `metadata`: a symlinked output path would make a
    // failed restore delete the user's link and leave the plaintext behind in
    // whatever directory the link pointed at.
    let metadata = match fs::symlink_metadata(output) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| format!("could not check {}", output.display()));
        }
        Ok(metadata) => metadata,
    };
    if metadata.file_type().is_symlink() {
        bail!(
            "{} is a symbolic link; envrelay only restores into a real directory \
             it can clean up safely on failure, so pass the target directory itself",
            output.display()
        );
    }
    if !metadata.is_dir() {
        bail!("{} exists and is not a directory", output.display());
    }
    let is_empty = fs::read_dir(output)
        .with_context(|| format!("could not read {}", output.display()))?
        .next()
        .is_none();
    if !is_empty {
        bail!(
            "output directory {} is not empty; \
             envrelay only restores into a new or empty directory",
            output.display()
        );
    }
    Ok(true)
}

fn pack(file: File, staging_dir: &Path, output: &Path, passphrase: SecretString) -> Result<()> {
    let encryptor = age::Encryptor::with_user_passphrase(passphrase);
    let encrypting = encryptor
        .wrap_output(file)
        .context("could not start encryption")?;
    let compressing =
        zstd::Encoder::new(encrypting, ZSTD_LEVEL).context("could not start compression")?;

    let mut archive = tar::Builder::new(compressing);
    // Store symlinks as symlinks. A development environment is full of them,
    // and following them would both duplicate data and fail on dangling ones.
    archive.follow_symlinks(false);
    // The empty prefix makes archive paths relative to the staging root, which
    // is what the `archive` fields in manifest.json refer to.
    archive
        .append_dir_all("", staging_dir)
        .with_context(|| format!("could not pack {}", staging_dir.display()))?;

    let compressing = archive
        .into_inner()
        .context("could not finish the archive")?;
    let encrypting = compressing
        .finish()
        .context("could not finish compression")?;
    let file = encrypting.finish().context("could not finish encryption")?;
    file.sync_all()
        .with_context(|| format!("could not flush {} to disk", output.display()))?;
    Ok(())
}

/// Opens the decrypt-and-decompress pipeline over a backup file. The
/// returned reader yields the plain tar stream.
fn open_stream(backup_file: &Path, passphrase: SecretString) -> Result<impl Read> {
    let file = File::open(backup_file)
        .with_context(|| format!("could not open {}", backup_file.display()))?;

    let decryptor = age::Decryptor::new_buffered(BufReader::new(file)).map_err(explain)?;
    let identity = age::scrypt::Identity::new(passphrase);
    let decrypting = decryptor
        .decrypt(iter::once(&identity as &dyn age::Identity))
        .map_err(explain)?;
    zstd::Decoder::new(decrypting).context("could not start decompression")
}

fn unpack(backup_file: &Path, output: &Path, passphrase: SecretString) -> Result<()> {
    let decompressing = open_stream(backup_file, passphrase)?;
    let mut archive = tar::Archive::new(decompressing);
    archive.set_preserve_permissions(true);

    for entry in archive.entries().context("could not read the archive")? {
        let mut entry = entry.context("could not read the next file in the archive")?;
        let path = entry
            .path()
            .context("an archive entry has an unreadable path")?
            .into_owned();
        check_archive_path(&path)?;

        let unpacked = entry
            .unpack_in(output)
            .with_context(|| format!("could not restore {}", path.display()))?;
        // `unpack_in` reports a refusal by returning false rather than by
        // failing, which would leave a silently incomplete restore behind.
        if !unpacked {
            bail!(
                "the archive entry {} was refused by the unpacker; \
                 refusing to restore this backup",
                path.display()
            );
        }
    }
    Ok(())
}

/// Rejects any entry that could write outside the output directory.
///
/// `tar`'s own `unpack_in` silently *skips* such entries. A backup that quietly
/// drops files is worse than one that fails loudly, so they are caught here.
fn check_archive_path(path: &Path) -> Result<()> {
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "the archive entry {} points outside the output directory; \
                     refusing to restore this backup",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

/// Turns an age failure into something a person can act on.
fn explain(error: age::DecryptError) -> anyhow::Error {
    use age::DecryptError::{
        DecryptionFailed, ExcessiveWork, InvalidHeader, InvalidMac, Io, KeyDecryptionFailed,
        NoMatchingKeys, UnknownFormat,
    };

    match error {
        NoMatchingKeys | DecryptionFailed | KeyDecryptionFailed | InvalidMac => {
            anyhow!("passphrase incorrect or file corrupted")
        }
        InvalidHeader | UnknownFormat => {
            anyhow!("this file is not an envrelay backup")
        }
        ExcessiveWork { required, target } => anyhow!(
            "this file asks for far more work to open than envrelay ever writes \
             (2^{required} against this machine's 2^{target}); it was not made by envrelay"
        ),
        Io(error) => anyhow::Error::new(error).context("could not read the backup file"),
    }
}
