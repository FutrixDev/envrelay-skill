//! End-to-end tests for the two commands.
//!
//! Every test drives the real binary, because the promises being checked here
//! ("exit non-zero", "leave nothing behind") are promises about the process,
//! not about a function.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::TempDir;

const PASSPHRASE: &str = "correct horse battery staple";

fn envrelay() -> Command {
    Command::cargo_bin("envrelay").expect("the envrelay binary should be built")
}

/// Writes a passphrase file and returns its path. Real backups never do this;
/// tests have no terminal to type into.
fn passphrase_file(dir: &Path, passphrase: &str) -> PathBuf {
    let path = dir.join(format!("pass-{passphrase:.8}.txt"));
    fs::write(&path, format!("{passphrase}\n")).unwrap();
    path
}

fn mode_of(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}

/// A staging tree with the shapes that a real backup contains: nested
/// directories, a private file, an executable, an empty file, a file large
/// enough to cross the streaming chunk boundaries, and a symlink.
fn build_staging_tree(root: &Path) -> Vec<PathBuf> {
    fs::create_dir_all(root.join("files/home/.config/nested")).unwrap();
    fs::create_dir_all(root.join("credentials/ssh")).unwrap();

    fs::write(root.join("manifest.json"), br#"{"envrelay_manifest":1}"#).unwrap();
    fs::write(root.join("files/home/.zshrc"), b"export EDITOR=vim\n").unwrap();
    fs::write(root.join("files/home/.config/nested/deep.toml"), b"a = 1\n").unwrap();
    fs::write(root.join("files/home/empty"), b"").unwrap();
    fs::write(root.join("credentials/ssh/id_ed25519"), b"PRIVATE KEY\n").unwrap();

    let mut executable = fs::File::create(root.join("files/home/bin-script")).unwrap();
    executable.write_all(b"#!/bin/sh\necho hi\n").unwrap();
    drop(executable);

    // ~4 MiB of non-repeating bytes, so a truncated or reordered stream cannot
    // pass a byte comparison by accident.
    let large: Vec<u8> = (0..4 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    fs::write(root.join("files/home/large.bin"), &large).unwrap();

    std::os::unix::fs::symlink("../home/.zshrc", root.join("files/link-to-zshrc")).unwrap();

    fs::set_permissions(
        root.join("credentials/ssh/id_ed25519"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::set_permissions(
        root.join("credentials/ssh"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::set_permissions(
        root.join("files/home/bin-script"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::set_permissions(
        root.join("files/home/.zshrc"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();

    vec![
        PathBuf::from("manifest.json"),
        PathBuf::from("files/home/.zshrc"),
        PathBuf::from("files/home/.config/nested/deep.toml"),
        PathBuf::from("files/home/empty"),
        PathBuf::from("files/home/bin-script"),
        PathBuf::from("files/home/large.bin"),
        PathBuf::from("credentials/ssh/id_ed25519"),
    ]
}

#[test]
fn roundtrip_preserves_bytes_and_permission_bits() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let restored = work.path().join("restored");
    let pass = passphrase_file(work.path(), PASSPHRASE);

    let files = build_staging_tree(&staging);

    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();

    envrelay()
        .args(["decrypt"])
        .arg(&backup)
        .arg("-o")
        .arg(&restored)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();

    for relative in &files {
        let before = staging.join(relative);
        let after = restored.join(relative);
        assert_eq!(
            fs::read(&before).unwrap(),
            fs::read(&after).unwrap(),
            "contents differ for {}",
            relative.display()
        );
        assert_eq!(
            mode_of(&before),
            mode_of(&after),
            "permission bits differ for {}",
            relative.display()
        );
    }

    assert_eq!(
        mode_of(&staging.join("credentials/ssh")),
        mode_of(&restored.join("credentials/ssh")),
        "directory permission bits differ"
    );

    assert_eq!(
        mode_of(&backup),
        0o600,
        "the backup file carries SSH keys and must be private to its owner"
    );

    assert_eq!(
        mode_of(&restored),
        0o700,
        "a restore directory created by envrelay holds plaintext credentials \
         and must be private to its owner"
    );

    let link = restored.join("files/link-to-zshrc");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "a symlink should be restored as a symlink, not as a copy of its target"
    );
    assert_eq!(
        fs::read_link(&link).unwrap(),
        Path::new("../home/.zshrc"),
        "the symlink should still point where it did"
    );
}

#[test]
fn a_backup_can_be_opened_by_standard_age_and_tar_tools() {
    // The escape hatch is a promise: the format has to be a plain age(scrypt)
    // stream, so `Decryptor` + `zstd` + `tar` used directly -- with no envrelay
    // code in the path -- must be able to read it.
    use std::io::Read;

    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let pass = passphrase_file(work.path(), PASSPHRASE);
    build_staging_tree(&staging);

    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();

    let file = fs::File::open(&backup).unwrap();
    let decryptor = age::Decryptor::new_buffered(std::io::BufReader::new(file)).unwrap();
    let identity =
        age::scrypt::Identity::new(age::secrecy::SecretString::from(PASSPHRASE.to_owned()));
    let plaintext = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .unwrap();
    let mut archive = tar::Archive::new(zstd::Decoder::new(plaintext).unwrap());

    let mut manifest = None;
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        if entry.path().unwrap() == Path::new("manifest.json") {
            let mut contents = String::new();
            entry.read_to_string(&mut contents).unwrap();
            manifest = Some(contents);
        }
    }
    assert_eq!(manifest.as_deref(), Some(r#"{"envrelay_manifest":1}"#));
}

#[test]
fn the_wrong_passphrase_fails_and_leaves_no_output_behind() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let restored = work.path().join("restored");
    let right = passphrase_file(work.path(), PASSPHRASE);
    let wrong = passphrase_file(work.path(), "not the passphrase");

    build_staging_tree(&staging);
    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&right)
        .assert()
        .success();

    envrelay()
        .args(["decrypt"])
        .arg(&backup)
        .arg("-o")
        .arg(&restored)
        .arg("--passphrase-file")
        .arg(&wrong)
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "passphrase incorrect or file corrupted",
        ));

    assert!(
        !restored.exists(),
        "a failed decrypt must not leave a half-restored directory behind"
    );
}

#[test]
fn a_corrupted_backup_fails_with_a_clear_message_and_no_residue() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let restored = work.path().join("restored");
    let pass = passphrase_file(work.path(), PASSPHRASE);

    build_staging_tree(&staging);
    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();

    // Flip one byte in the middle of the payload: the header still parses and
    // the passphrase still checks out, but the STREAM MAC fails mid-restore.
    let mut bytes = fs::read(&backup).unwrap();
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0xff;
    fs::remove_file(&backup).unwrap();
    fs::write(&backup, bytes).unwrap();

    envrelay()
        .args(["decrypt"])
        .arg(&backup)
        .arg("-o")
        .arg(&restored)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "the backup file is corrupted or truncated",
        ));

    assert!(
        !restored.exists(),
        "a failed decrypt must not leave a half-restored directory behind"
    );
}

#[test]
fn a_failed_decrypt_leaves_a_pre_existing_empty_directory_in_place() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let restored = work.path().join("restored");
    let right = passphrase_file(work.path(), PASSPHRASE);
    let wrong = passphrase_file(work.path(), "not the passphrase");

    build_staging_tree(&staging);
    fs::create_dir(&restored).unwrap();
    fs::set_permissions(&restored, fs::Permissions::from_mode(0o750)).unwrap();
    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&right)
        .assert()
        .success();

    envrelay()
        .args(["decrypt"])
        .arg(&backup)
        .arg("-o")
        .arg(&restored)
        .arg("--passphrase-file")
        .arg(&wrong)
        .assert()
        .failure();

    assert!(restored.is_dir(), "the user's own directory should survive");
    assert_eq!(fs::read_dir(&restored).unwrap().count(), 0);
    assert_eq!(
        mode_of(&restored),
        0o750,
        "the user's own directory should come back with the mode they gave it"
    );
}

#[test]
fn encrypt_refuses_an_empty_staging_directory() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let pass = passphrase_file(work.path(), PASSPHRASE);
    fs::create_dir(&staging).unwrap();

    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(work.path().join("backup.envrelay"))
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .failure()
        .stderr(predicates::str::contains("is empty"));

    assert!(!work.path().join("backup.envrelay").exists());
}

#[test]
fn encrypt_refuses_a_staging_directory_that_does_not_exist() {
    let work = TempDir::new().unwrap();
    let pass = passphrase_file(work.path(), PASSPHRASE);

    envrelay()
        .args(["encrypt"])
        .arg(work.path().join("nowhere"))
        .arg("-o")
        .arg(work.path().join("backup.envrelay"))
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .failure()
        .stderr(predicates::str::contains("not readable"));
}

#[test]
fn encrypt_refuses_to_overwrite_an_existing_output_file() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let pass = passphrase_file(work.path(), PASSPHRASE);

    build_staging_tree(&staging);
    fs::write(&backup, b"someone else's file").unwrap();

    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .failure()
        .stderr(predicates::str::contains("already exists"));

    assert_eq!(
        fs::read(&backup).unwrap(),
        b"someone else's file",
        "refusing to overwrite must also mean refusing to delete"
    );
}

#[test]
fn encrypt_refuses_an_output_path_inside_the_staging_directory() {
    // Writing the backup into the tree being packed would make tar read the
    // file it is producing, growing it forever.
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let pass = passphrase_file(work.path(), PASSPHRASE);
    build_staging_tree(&staging);

    for output in [
        staging.join("backup.envrelay"),
        staging.join("files/backup.envrelay"),
    ] {
        envrelay()
            .args(["encrypt"])
            .arg(&staging)
            .arg("-o")
            .arg(&output)
            .arg("--passphrase-file")
            .arg(&pass)
            .assert()
            .failure()
            .stderr(predicates::str::contains(
                "inside the staging directory being packed",
            ));
        assert!(!output.exists(), "nothing should be written");
    }
}

#[test]
fn decrypt_refuses_a_symlinked_output_directory() {
    // A symlinked output path would make a failed restore delete the user's
    // link while the plaintext stays behind in the directory it pointed at.
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let target = work.path().join("real-dir");
    let restored = work.path().join("restored-link");
    let pass = passphrase_file(work.path(), PASSPHRASE);

    build_staging_tree(&staging);
    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();

    fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, &restored).unwrap();

    envrelay()
        .args(["decrypt"])
        .arg(&backup)
        .arg("-o")
        .arg(&restored)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .failure()
        .stderr(predicates::str::contains("is a symbolic link"));

    assert!(
        fs::symlink_metadata(&restored)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the user's symlink should survive untouched"
    );
    assert_eq!(
        fs::read_dir(&target).unwrap().count(),
        0,
        "nothing should be restored through the link"
    );
}

#[test]
fn a_failed_decrypt_removes_every_directory_it_created() {
    // The output path may be several directories deep; a failed restore has to
    // take the whole chain it created back out, not just the innermost one.
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let restored = work.path().join("a/b/c/restored");
    let right = passphrase_file(work.path(), PASSPHRASE);
    let wrong = passphrase_file(work.path(), "not the passphrase");

    build_staging_tree(&staging);
    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&right)
        .assert()
        .success();

    envrelay()
        .args(["decrypt"])
        .arg(&backup)
        .arg("-o")
        .arg(&restored)
        .arg("--passphrase-file")
        .arg(&wrong)
        .assert()
        .failure();

    assert!(
        !work.path().join("a").exists(),
        "the intermediate directories created for the restore should be gone"
    );
}

#[test]
fn a_failed_decrypt_with_a_relative_output_path_still_cleans_up() {
    // `Path::ancestors` on a relative path bottoms out in an empty path that
    // never exists; the cleanup logic must not trip over that and leave the
    // partially restored plaintext behind.
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let right = passphrase_file(work.path(), PASSPHRASE);
    let wrong = passphrase_file(work.path(), "not the passphrase");

    build_staging_tree(&staging);
    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&right)
        .assert()
        .success();

    for relative_output in ["restored", "deep/nested/restored"] {
        envrelay()
            .current_dir(work.path())
            .args(["decrypt"])
            .arg(&backup)
            .args(["-o", relative_output])
            .arg("--passphrase-file")
            .arg(&wrong)
            .assert()
            .failure();

        let top = Path::new(relative_output).components().next().unwrap();
        assert!(
            !work.path().join(top.as_os_str()).exists(),
            "{relative_output}: a failed decrypt must clean up relative output paths too"
        );
    }
}

#[test]
fn decrypt_refuses_a_non_empty_output_directory() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let restored = work.path().join("restored");
    let pass = passphrase_file(work.path(), PASSPHRASE);

    build_staging_tree(&staging);
    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();

    fs::create_dir(&restored).unwrap();
    fs::write(restored.join("existing.txt"), b"mine").unwrap();

    envrelay()
        .args(["decrypt"])
        .arg(&backup)
        .arg("-o")
        .arg(&restored)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .failure()
        .stderr(predicates::str::contains("is not empty"));

    assert_eq!(fs::read(restored.join("existing.txt")).unwrap(), b"mine");
}

#[test]
fn decrypt_refuses_an_archive_entry_that_escapes_the_output_directory() {
    for (label, entry_name) in [("parent", "../evil"), ("absolute", "/tmp/evil")] {
        let work = TempDir::new().unwrap();
        let backup = work.path().join("hostile.envrelay");
        let restored = work.path().join("restored");
        let pass = passphrase_file(work.path(), PASSPHRASE);

        write_hostile_backup(&backup, entry_name);

        envrelay()
            .args(["decrypt"])
            .arg(&backup)
            .arg("-o")
            .arg(&restored)
            .arg("--passphrase-file")
            .arg(&pass)
            .assert()
            .failure()
            .stderr(predicates::str::contains(
                "points outside the output directory",
            ));

        assert!(!restored.exists(), "{label}: nothing should be restored");
        assert!(
            !Path::new("/tmp/evil").exists(),
            "{label}: nothing should be written outside the output directory"
        );
    }
}

#[test]
fn inspect_lists_entries_and_prints_the_manifest_without_writing() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let pass = passphrase_file(work.path(), PASSPHRASE);
    build_staging_tree(&staging);

    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();

    // An empty working directory makes "writes nothing to disk" checkable:
    // anything inspect left behind would show up here.
    let probe = work.path().join("probe");
    fs::create_dir(&probe).unwrap();

    envrelay()
        .current_dir(&probe)
        .args(["inspect"])
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success()
        // "PRIVATE KEY\n" is 12 bytes, so the whole line is deterministic:
        // kind, the 0600 mode, the right-aligned size, the path.
        .stdout(predicates::str::contains(
            "-0600           12 credentials/ssh/id_ed25519",
        ))
        .stdout(predicates::str::contains(
            "files/link-to-zshrc -> ../home/.zshrc",
        ))
        .stdout(predicates::str::contains("--- manifest.json ---"))
        .stdout(predicates::str::contains(r#"{"envrelay_manifest":1}"#));

    assert_eq!(
        fs::read_dir(&probe).unwrap().count(),
        0,
        "inspect promises to write nothing to disk"
    );
}

#[test]
fn verify_confirms_an_intact_backup_and_writes_nothing() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let pass = passphrase_file(work.path(), PASSPHRASE);
    build_staging_tree(&staging);

    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();

    let probe = work.path().join("probe");
    fs::create_dir(&probe).unwrap();

    envrelay()
        .current_dir(&probe)
        .args(["verify"])
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success()
        .stdout(predicates::str::contains("ok: "))
        .stdout(predicates::str::contains(
            "the passphrase is correct and the whole file is intact",
        ));

    assert_eq!(
        fs::read_dir(&probe).unwrap().count(),
        0,
        "verify promises to write nothing to disk"
    );
}

#[test]
fn verify_and_inspect_fail_on_a_corrupted_or_truncated_backup() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let pass = passphrase_file(work.path(), PASSPHRASE);
    build_staging_tree(&staging);

    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&pass)
        .assert()
        .success();
    let intact = fs::read(&backup).unwrap();

    // One flipped byte mid-payload, and a file cut short: both must fail,
    // because "verify said ok" is the promise a user restores on.
    let mut flipped = intact.clone();
    let middle = flipped.len() / 2;
    flipped[middle] ^= 0xff;
    let mut truncated = intact.clone();
    truncated.truncate(intact.len() - 37);

    for (label, bytes) in [("flipped", flipped), ("truncated", truncated)] {
        let bad = work.path().join(format!("{label}.envrelay"));
        fs::write(&bad, bytes).unwrap();

        // inspect must fail on the same damage: a listing that prints in full
        // and exits clean would read as "the file is good".
        for subcommand in ["verify", "inspect"] {
            envrelay()
                .args([subcommand])
                .arg(&bad)
                .arg("--passphrase-file")
                .arg(&pass)
                .assert()
                .failure()
                .stderr(predicates::str::contains(
                    "the backup file is corrupted or truncated",
                ));
        }
    }
}

#[test]
fn inspect_and_verify_fail_with_the_wrong_passphrase() {
    let work = TempDir::new().unwrap();
    let staging = work.path().join("staging");
    let backup = work.path().join("backup.envrelay");
    let right = passphrase_file(work.path(), PASSPHRASE);
    let wrong = passphrase_file(work.path(), "not the passphrase");
    build_staging_tree(&staging);

    envrelay()
        .args(["encrypt"])
        .arg(&staging)
        .arg("-o")
        .arg(&backup)
        .arg("--passphrase-file")
        .arg(&right)
        .assert()
        .success();

    for subcommand in ["inspect", "verify"] {
        envrelay()
            .args([subcommand])
            .arg(&backup)
            .arg("--passphrase-file")
            .arg(&wrong)
            .assert()
            .failure()
            .stderr(predicates::str::contains(
                "passphrase incorrect or file corrupted",
            ));
    }
}

/// Builds a well-formed backup whose archive contains one entry with a path
/// that would escape the output directory.
///
/// `tar::Header::set_path` refuses `..` outright, so the name is written into
/// the header by hand -- which is exactly what a hostile archive would do.
fn write_hostile_backup(output: &Path, entry_name: &str) {
    let payload = b"pwned\n";

    let mut header = tar::Header::new_gnu();
    header.set_size(payload.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    let name = entry_name.as_bytes();
    header.as_gnu_mut().unwrap().name[..name.len()].copy_from_slice(name);
    header.set_cksum();

    let file = fs::File::create(output).unwrap();
    let encryptor = age::Encryptor::with_user_passphrase(age::secrecy::SecretString::from(
        PASSPHRASE.to_owned(),
    ));
    let encrypting = encryptor.wrap_output(file).unwrap();
    let compressing = zstd::Encoder::new(encrypting, 3).unwrap();
    let mut archive = tar::Builder::new(compressing);
    archive.append(&header, &payload[..]).unwrap();
    archive
        .into_inner()
        .unwrap()
        .finish()
        .unwrap()
        .finish()
        .unwrap();
}
