use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Seek},
    ops::{Deref, DerefMut},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
};

use age::{
    armor::{ArmoredReader, ArmoredWriter},
    Decryptor, Identity, Recipient,
};
use blake3::Hash;
use eyre::{bail, Context};
use tracing::{debug, error, info};

use crate::{config::Config, util::random_alpha_num};

struct TempFile {
    path: PathBuf,
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
        let mut tmp_name = OsString::from(random_alpha_num());

        let ext = secret_path
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(|filename| {
                let without_pem = filename.strip_suffix(".pem").unwrap_or(filename);

                without_pem.rsplit_once(".")
            })
            .map(|(_file, ext)| ext)
            .filter(|ext| !ext.is_empty());

        if let Some(ext) = ext {
            tmp_name.push(".");
            tmp_name.push(ext);
        }

        let tpm_path = cache_dir.join(tmp_name);

        Self { path: tpm_path }
    }

    fn open(&self) -> eyre::Result<File> {
        File::options()
            .read(true)
            .mode(0o600)
            .open(&self.path)
            .wrap_err("couldn't open temporary file")
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            error!(error = %err, "couln't remove temporary file");
        }
    }
}

struct SecretFile {
    is_empty: bool,
    file: File,
}

impl SecretFile {
    fn open(path: &Path) -> eyre::Result<Self> {
        debug!(path = %path.display(), "opening secret file");

        let file = File::options()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .wrap_err("couldn't open secret file")?;

        Ok(Self {
            is_empty: file.metadata()?.size() == 0,
            file,
        })
    }

    fn maybe_decrypt(&mut self, config: &Config, mut tmp_file: File) -> eyre::Result<Option<Hash>> {
        if self.is_empty {
            debug!("secret file is empty");

            return Ok(None);
        }

        info!("decrypting existing file");

        let identities = config.secrets.identity()?;

        let decryptor = Decryptor::new(ArmoredReader::new(&self.file))?;
        let mut stream = decryptor.decrypt(std::iter::once(&identities as &dyn Identity))?;

        io::copy(&mut stream, &mut tmp_file)?;

        self.file.rewind()?;

        tmp_file.sync_all()?;
        tmp_file.rewind()?;

        let hash = blake3::Hasher::new().update_reader(&tmp_file)?.finalize();

        Ok(Some(hash))
    }

    fn encrypt(self, config: &Config, mut tmp_file: File) -> eyre::Result<()> {
        let recipients = config.secrets.recipients()?;
        let recipients = recipients.iter().map(|r| r as &dyn Recipient);

        let encriptor = age::Encryptor::with_recipients(recipients)?;
        let mut writer = encriptor.wrap_output(ArmoredWriter::wrap_output(
            &self.file,
            age::armor::Format::AsciiArmor,
        )?)?;

        io::copy(&mut tmp_file, &mut writer)?;

        writer.finish().and_then(|armor| armor.finish())?;

        Ok(())
    }
}

impl Deref for SecretFile {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl DerefMut for SecretFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file
    }
}

pub fn edit(secret_path: &Path) -> eyre::Result<()> {
    let config = crate::config();
    let cache_dir = config.dirs.cache()?;

    let mut secret_file = SecretFile::open(secret_path)?;
    let tmp_file = TempFile::fom_secret(secret_path, cache_dir);

    let open_tmp_file = tmp_file.create()?;
    let hash = secret_file.maybe_decrypt(config, open_tmp_file)?;

    let out = Command::new(&config.editor)
        .arg(&tmp_file.path)
        .spawn()?
        .wait_with_output()?;

    if !out.status.success() {
        error!(
            status = out.status.code(),
            "editor exited with an error status code"
        );

        bail!("editor exited with an error");
    }

    let mut open_tmp_file = tmp_file.open()?;

    if let Some(hash) = hash {
        let new = blake3::Hasher::new()
            .update_reader(&open_tmp_file)?
            .finalize();

        if hash == new {
            info!("the file is still the same");

            return Ok(());
        }

        open_tmp_file.rewind()?;
    }

    secret_file.encrypt(config, open_tmp_file)?;

    Ok(())
}
