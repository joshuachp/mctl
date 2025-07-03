use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, stdin, stdout},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use age::{
    Decryptor, Identity, Recipient,
    armor::{ArmoredReader, ArmoredWriter},
};
use blake3::Hash;
use color_eyre::{Section, owo_colors::OwoColorize};
use eyre::{Context, bail, eyre};
use tracing::{debug, error, info};

use crate::{config::Config, util::random_alpha_num};

fn encrypt<R, W>(config: &Config, reader: &mut R, writer: &mut W) -> eyre::Result<()>
where
    R: std::io::Read,
    W: std::io::Write,
{
    let recipients = config.secrets.recipients()?;
    let recipients = recipients.iter().map(|r| r as &dyn Recipient);

    let encriptor = age::Encryptor::with_recipients(recipients)?;
    let mut writer = encriptor.wrap_output(ArmoredWriter::wrap_output(
        writer,
        age::armor::Format::AsciiArmor,
    )?)?;

    io::copy(reader, &mut writer)?;

    writer.finish().and_then(|armor| armor.finish())?;

    Ok(())
}

fn decrypt<R, W>(config: &Config, reader: &mut R, dst: &mut W) -> eyre::Result<()>
where
    R: std::io::Read,
    W: std::io::Write,
{
    let identities = config.secrets.identity()?;

    let decryptor = Decryptor::new(ArmoredReader::new(reader))?;
    let mut stream = decryptor.decrypt(std::iter::once(&identities as &dyn Identity))?;

    io::copy(&mut stream, dst).wrap_err("couldn't copy to destination")?;

    Ok(())
}

struct TempFile {
    path: PathBuf,
    // Hash before an edit
    hash: Option<Hash>,
}

impl TempFile {
    fn create(&self) -> eyre::Result<File> {
        debug!(path = %self.path.display(), "creating temporary file");

        let file = File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .mode(0o600)
            .open(&self.path)
            .wrap_err("couldn't open temporary file")?;

        Ok(file)
    }

    fn fom_secret(secret_path: &Path, cache_dir: &Path) -> Self {
        let ext = secret_path
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(|filename| {
                let without_pem = filename.strip_suffix(".pem").unwrap_or(filename);

                without_pem.rsplit_once(".")
            })
            .map(|(_file, ext)| ext)
            .filter(|ext| !ext.is_empty());

        Self::new(cache_dir, ext)
    }

    fn open(&self) -> eyre::Result<File> {
        File::options()
            .read(true)
            .mode(0o600)
            .open(&self.path)
            .wrap_err("couldn't open temporary file")
    }

    fn new(cache_dir: &Path, ext: Option<&str>) -> Self {
        let mut tmp_name = OsString::from(random_alpha_num());

        if let Some(ext) = ext {
            tmp_name.push(".");
            tmp_name.push(ext);
        }

        let tpm_path = cache_dir.join(tmp_name);

        Self {
            path: tpm_path,
            hash: None,
        }
    }

    fn hash(&self) -> Option<Hash> {
        if !self.path.exists() {
            return None;
        }

        let file = self.open().ok()?;

        let hash = blake3::Hasher::new().update_reader(&file).ok()?.finalize();

        Some(hash)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            error!(error = %err, "couln't remove temporary file");
        }
    }
}

struct SecretFile<'a> {
    path: &'a Path,
    allow_empty: bool,
}

impl<'a> SecretFile<'a> {
    fn new(path: &'a Path, allow_empty: bool) -> Self {
        Self { path, allow_empty }
    }

    fn open(&self, truncate: bool) -> eyre::Result<File> {
        debug!(path = %self.path.display(), "opening secret file");

        File::options()
            .create(true)
            .truncate(truncate)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(self.path)
            .wrap_err("couldn't open secret file")
    }

    fn decrypt_to<W>(&self, config: &Config, dst: &mut W) -> eyre::Result<()>
    where
        W: std::io::Write,
    {
        let mut file = self.open(false)?;

        decrypt(config, &mut file, dst)
    }

    fn encrypt_from<R>(&self, config: &Config, reader: &mut R) -> eyre::Result<()>
    where
        R: std::io::Read,
    {
        let mut file = self.open(true)?;

        encrypt(config, reader, &mut file)?;

        file.sync_all()?;

        Ok(())
    }

    /// Decrypts the secret to a temp file, returning the hash if the secret already exists
    fn decrypt_to_tmp(&self, config: &Config) -> eyre::Result<TempFile> {
        let mut tmp = TempFile::fom_secret(self.path, config.dirs.cache()?);

        if !self.path.try_exists()? {
            info!("new secret file");

            return Ok(tmp);
        }

        info!("decrypting secret file");

        let mut tmp_file = tmp.create()?;

        self.decrypt_to(config, &mut tmp_file)
            .wrap_err("couldn't decrypt to temp file")?;

        tmp_file.sync_all()?;

        // Update the hash
        tmp.hash = tmp.hash();

        Ok(tmp)
    }

    /// Encrypts the content of a temp file, only if the hash differs.
    fn encrypt_from_tmp(&self, config: &Config, tmp: TempFile) -> eyre::Result<()> {
        if tmp.path.metadata()?.len() == 0 && !self.allow_empty {
            return Err(eyre!("secrets cannot be empty")).note(format!(
                "you can pass the {} option to create an empty secret",
                "--allow-empty".blue()
            ));
        }

        if let Some(hash) = tmp.hash {
            let new = tmp.hash();

            if new.is_some_and(|new| hash == new) {
                info!("the file is still the same");

                return Ok(());
            }
        }

        info!("encrypt the secret file");

        let mut tmp_file = tmp.open()?;

        self.encrypt_from(config, &mut tmp_file)
            .wrap_err("couldn't decrypt to temp file")?;

        Ok(())
    }

    fn rotate(&self, config: &Config) -> eyre::Result<()> {
        let mut tmp = self.decrypt_to_tmp(&config)?;
        // Force re-encryption
        tmp.hash.take();
        self.encrypt_from_tmp(&config, tmp)?;

        Ok(())
    }
}

pub fn edit(secret_path: &Path, allow_empty: bool) -> eyre::Result<()> {
    let config = crate::config();

    let secret_file = SecretFile::new(secret_path, allow_empty);

    let tmp = secret_file.decrypt_to_tmp(config)?;

    let out = Command::new(&config.editor)
        .arg(&tmp.path)
        .spawn()?
        .wait_with_output()?;

    if !out.status.success() {
        error!(
            status = out.status.code(),
            "editor exited with an error status code"
        );

        bail!("editor exited with an error");
    }

    secret_file.encrypt_from_tmp(config, tmp)?;

    Ok(())
}

pub fn from_stdin(allow_empty: bool, file: &Path) -> eyre::Result<()> {
    let config = crate::config();

    let mut stdin = stdin().lock();

    let tmp = TempFile::new(config.dirs.cache()?, None);

    SecretFile::new(&tmp.path, allow_empty).encrypt_from(config, &mut stdin)?;

    fs::copy(&tmp.path, file).wrap_err("coudl't copy temp file")?;

    info!("secret encrypted");

    Ok(())
}

pub fn cat(file: &Path) -> eyre::Result<()> {
    let config = crate::config();

    let mut stdout = stdout().lock();

    SecretFile::new(file, true)
        .decrypt_to(config, &mut stdout)
        .wrap_err("couldn't decrypt to stdout")?;

    Ok(())
}

pub fn rotate(file: &Path) -> eyre::Result<()> {
    let config = crate::config();

    SecretFile::new(file, true).rotate(config)?;

    info!("secret encrypted");

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pretty_assertions::assert_ne;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn encrypt_and_decrypt() {
        let tmp = TempDir::new().unwrap();

        let file = tmp.path().join("secret.txt.pem");

        let plaintext = "Hello World!";
        let mut reader = Cursor::new(plaintext);

        let config = Config::mock();

        SecretFile::new(&file, false)
            .encrypt_from(&config, &mut reader)
            .unwrap();

        let mut out = Cursor::new(Vec::new());

        SecretFile::new(&file, false)
            .decrypt_to(&config, &mut out)
            .unwrap();

        let inner = out.into_inner();
        let out = str::from_utf8(&inner).unwrap();

        assert_eq!(out, plaintext);
    }

    #[test]
    fn encrypt_and_decrypt_smaller() {
        let tmp = TempDir::new().unwrap();

        let file = tmp.path().join("secret.txt.pem");

        let config = Config::mock();

        let plaintext = "Hello World!";
        let mut reader = Cursor::new(plaintext);

        SecretFile::new(&file, false)
            .encrypt_from(&config, &mut reader)
            .unwrap();

        let plaintext = "Hello";
        let mut reader = Cursor::new(plaintext);

        SecretFile::new(&file, false)
            .encrypt_from(&config, &mut reader)
            .unwrap();

        let mut out = Cursor::new(Vec::new());

        SecretFile::new(&file, false)
            .decrypt_to(&config, &mut out)
            .unwrap();

        let inner = out.into_inner();
        let out = str::from_utf8(&inner).unwrap();

        assert_eq!(out, plaintext);
    }

    #[test]
    fn encrypt_and_decrypt_bigger() {
        let tmp = TempDir::new().unwrap();

        let file = tmp.path().join("secret.txt.pem");

        let config = Config::mock();

        let plaintext = "Hello";
        let mut reader = Cursor::new(plaintext);

        SecretFile::new(&file, false)
            .encrypt_from(&config, &mut reader)
            .unwrap();

        let plaintext = "Hello world";
        let mut reader = Cursor::new(plaintext);

        SecretFile::new(&file, false)
            .encrypt_from(&config, &mut reader)
            .unwrap();

        let mut out = Cursor::new(Vec::new());

        SecretFile::new(&file, false)
            .decrypt_to(&config, &mut out)
            .unwrap();

        let inner = out.into_inner();
        let out = str::from_utf8(&inner).unwrap();

        assert_eq!(out, plaintext);
    }

    #[test]
    fn encrypt_and_decrypt_smaller_big() {
        let tmp = TempDir::new().unwrap();

        let file = tmp.path().join("secret.txt.pem");

        let config = Config::mock();

        let plaintext = vec![b'a'; 2048];
        let mut reader = Cursor::new(plaintext);

        SecretFile::new(&file, false)
            .encrypt_from(&config, &mut reader)
            .unwrap();

        let plaintext = vec![b'b'; 10];
        let mut reader = Cursor::new(plaintext.clone());

        SecretFile::new(&file, false)
            .encrypt_from(&config, &mut reader)
            .unwrap();

        let mut out = Cursor::new(Vec::new());

        SecretFile::new(&file, false)
            .decrypt_to(&config, &mut out)
            .unwrap();

        let inner = out.into_inner();

        assert_eq!(inner, plaintext);
    }

    #[test]
    fn rotate_encrypted_file() {
        let tmp = TempDir::new().unwrap();

        let file = tmp.path().join("secret.txt.pem");

        let config = Config::mock();

        let plaintext = b"Hello world!";
        let mut reader = Cursor::new(plaintext);

        SecretFile::new(&file, false)
            .encrypt_from(&config, &mut reader)
            .unwrap();

        let before = fs::read_to_string(&file).unwrap();

        let config = config.use_other_recipent();

        SecretFile::new(&file, false).rotate(&config).unwrap();

        let after = fs::read_to_string(&file).unwrap();

        let mut out = Cursor::new(Vec::new());

        SecretFile::new(&file, false)
            .decrypt_to(&config, &mut out)
            .unwrap();

        let inner = out.into_inner();

        assert_eq!(inner, plaintext);
        assert_ne!(before, after);
    }
}
