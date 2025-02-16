use std::{
    env::VarError,
    fs::{self},
    io::{self},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use age::x25519::{Identity, Recipient};
use color_eyre::{owo_colors::OwoColorize, Section};
use config::FileFormat;
use eyre::{ensure, eyre, OptionExt, WrapErr};
use serde::Deserialize;
use tracing::{debug, error};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub(crate) editor: String,
    pub(crate) dirs: Directories,
    pub(crate) secrets: Secrets,
}

impl Config {
    pub fn read(custom_conf: Option<&Path>) -> eyre::Result<Self> {
        let config_dir = dirs::config_local_dir()
            .ok_or_eyre("couldn't determin configuration directory")?
            .join(env!("CARGO_PKG_NAME"));

        let main_config =
            config::File::from(config_dir.join("config.toml")).format(FileFormat::Toml);

        let drop_config = Self::read_config_dir(&config_dir.join("config.d"))?
            .into_iter()
            .map(|path| config::File::from(path).format(FileFormat::Toml))
            .collect::<Vec<_>>();

        let mut config = config::Config::builder()
            .add_source(main_config)
            .add_source(drop_config)
            .add_source(
                config::Environment::with_prefix("MCTL")
                    .separator("_")
                    .list_separator(","),
            );

        // Additional config file
        if let Some(custom) = custom_conf {
            config = config.add_source(config::File::from(custom).format(FileFormat::Toml));
        }

        let editor = Self::read_env("VISUAL").or_else(|| Self::read_env("EDITOR"));

        if let Some(editor) = editor {
            config = config.set_default("editor", editor)?;
        }

        config
            .build()
            .wrap_err("couldn't read the config")?
            .try_deserialize::<Config>()
            .wrap_err("coldn't read the configuration")?
            .validate()
    }

    fn validate(self) -> eyre::Result<Self> {
        if !self.dirs.repository.is_dir() {
            let err =
                eyre!("The repository directory must be set to a valid direcotry").note(format!(
                    "Make sure {} is a valid directory",
                    self.dirs.repository.display().blue()
                ));

            return Err(err);
        };

        check_private_file(&self.secrets.key_file)?;
        check_private_file(&self.secrets.recipients_file)?;

        self.secrets.identity()?;

        Ok(self)
    }

    fn read_env(key: &str) -> Option<String> {
        match std::env::var(key) {
            Ok(value) => Some(value),
            Err(VarError::NotPresent) => None,
            Err(VarError::NotUnicode(value)) => {
                error!(
                    value = %value.to_string_lossy(),
                    "environment variable {key} is not UTF-8"
                );

                None
            }
        }
    }

    fn read_config_dir(config_dir: &Path) -> eyre::Result<Vec<PathBuf>> {
        if !config_dir.is_dir() {
            fs::create_dir_all(config_dir).wrap_err_with(|| {
                format!(
                    "couldn't create the configuration directory: {}",
                    config_dir.display()
                )
            })?;

            return Ok(Vec::new());
        }

        let mut paths = fs::read_dir(config_dir)?
            .filter_map(|res| {
                let entry = match res {
                    Ok(entry) => entry,
                    Err(err) => {
                        return Some(Err(err));
                    }
                };

                let path = entry.path();

                // Filter Toml config files
                if path.extension().is_none_or(|ext| ext != "toml") {
                    return None;
                }

                // Filter non files
                if !path.is_file() {
                    return None;
                }

                Some(Ok(path))
            })
            .collect::<Result<Vec<PathBuf>, io::Error>>()
            .wrap_err_with(|| {
                format!(
                    "couldn't read configuration directory: {}",
                    config_dir.display()
                )
            })?;

        paths.sort_unstable();

        Ok(paths)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Secrets {
    #[serde(default = "default_key_file")]
    key_file: PathBuf,
    #[serde(default = "default_recipients_file")]
    recipients_file: PathBuf,
}

impl Secrets {
    pub(crate) fn identity(&self) -> eyre::Result<Identity> {
        debug!(file = %self.key_file.display(), "reading identity file");

        let content = zeroize::Zeroizing::new(fs::read_to_string(&self.key_file)?);
        let key = content
            .lines()
            .filter(|s| !(s.starts_with("#") || s.is_empty()))
            .take(1)
            .next()
            .ok_or_eyre("couldn't find key line in key file")?;

        Identity::from_str(key).map_err(|err| {
            eyre!("{err}")
                .wrap_err("couldn't read identity file")
                .note(format!(
                    "Make sure {} is a valid age private key",
                    self.key_file.display().blue()
                ))
        })
    }

    pub(crate) fn recipients(&self) -> eyre::Result<Vec<Recipient>> {
        debug!(file = %self.recipients_file.display(), "reading recipients file");

        fs::read_to_string(&self.recipients_file)
            .wrap_err("couldnt read recipients file")
            .and_then(|s| {
                s.lines()
                    .enumerate()
                    .filter(|(_, l)| !(l.is_empty() || l.starts_with("#")))
                    .map(|(n, l)| {
                        Recipient::from_str(l).map_err(|err| {
                            eyre!("{err}").wrap_err(format!("couldn't parse {n} recipient line"))
                        })
                    })
                    .collect::<eyre::Result<Vec<Recipient>>>()
                    .and_then(|recipients| {
                        ensure!(!recipients.is_empty(), "the recipients file is empty");

                        debug!("read {} recipients", recipients.len());

                        Ok(recipients)
                    })
            })
            .with_note(|| {
                format!(
                    "Make sure {} is a valid recipient file",
                    self.recipients_file.display().blue()
                )
            })
    }
}

impl Default for Secrets {
    fn default() -> Self {
        Self {
            key_file: default_key_file(),
            recipients_file: default_recipients_file(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Directories {
    /// Cache directory
    #[serde(default = "default_cache_dir")]
    cache: PathBuf,
    /// Path to the repository
    pub(crate) repository: PathBuf,
}

impl Directories {
    pub(crate) fn cache(&self) -> eyre::Result<&Path> {
        fs::create_dir_all(&self.cache).wrap_err_with(|| {
            format!("couldn't create cache directory: {}", self.cache.display())
        })?;

        Ok(&self.cache)
    }
}

fn default_key_file() -> PathBuf {
    let mut dir = default_config_dir();
    dir.push("age");
    dir.push("key.txt");

    dir
}

fn default_recipients_file() -> PathBuf {
    let mut dir = default_config_dir();
    dir.push("age");
    dir.push("recipients.txt");

    dir
}

fn default_config_dir() -> PathBuf {
    let mut dir = dirs::config_local_dir().unwrap_or_else(|| PathBuf::from("~/.config"));

    dir.push(env!("CARGO_PKG_NAME"));
    dir
}

fn default_cache_dir() -> PathBuf {
    let mut dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));

    dir.push("mctl");

    dir
}

fn check_private_file(path: &Path) -> Result<(), eyre::Error> {
    let md = path
        .metadata()
        .wrap_err("couldn't read file metadata")
        .with_note(|| format!("make sure {} is a file and readable", path.display().blue()))?;

    ensure!(md.is_file(), "{} must be a file", path.display());

    let mode = md.permissions().mode();
    if mode ^ 0o077 == 0 {
        let err = eyre!(
            "insecure permission on {} with mode: {:o}",
            path.display(),
            mode
        )
        .note(format!(
            "make sure to set the permissions of {} to 600",
            path.display().blue()
        ));
        return Err(err);
    }

    Ok(())
}
