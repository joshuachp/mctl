use std::{
    collections::{HashMap, VecDeque},
    fs::{self, DirBuilder, DirEntry, ReadDir},
    io,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};

use eyre::{OptionExt, WrapErr};
use serde::Deserialize;
use tracing::{debug, error, info, instrument, warn};

/// Mode directory configuration `.mode.toml`
#[derive(Debug, Clone, Deserialize)]
struct Mode {
    /// Mode of the directory
    dir: u32,
    /// Mode of the directory
    files: HashMap<String, FileAttrs>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileAttrs {
    mode: u32,
    rename: Option<String>,
}

/// Visits directories, maintains symlinks
///
/// ```txt
/// /first
///     /foo
///         file.3
///         file.4
///     /bar
///     file.1
///     file.2
/// visits:
///   - p: first [foo]
///   - file.1 [foo, bar]
/// ```
struct DirIter {
    current: ReadDir,
    /// Stack of dir in parent to visit
    stack: Vec<PathBuf>,
}

impl DirIter {
    fn read_next(&mut self) -> Option<eyre::Result<DirItem>> {
        self.current
            .by_ref()
            .filter_map(|entry| {
                entry
                    .wrap_err("couldn't read entry")
                    .and_then(|entry| Self::handle_entry(&mut self.stack, entry))
                    .transpose()
            })
            .next()
    }

    fn handle_entry(stack: &mut Vec<PathBuf>, entry: DirEntry) -> eyre::Result<Option<DirItem>> {
        let file_type = entry.file_type().wrap_err("couldn't get file type")?;
        let path = entry.path();

        let item = if file_type.is_dir() {
            stack.push(path.clone());

            DirItem::dir(path)
        } else if file_type.is_file() {
            DirItem::file(path)
        } else if file_type.is_symlink() {
            DirItem::symlink(path)
        } else {
            warn!(path = %entry.path().display(), ?file_type, "skipping entry");

            return Ok(None);
        };

        return Ok(Some(item));
    }

    fn find_next_dir(&mut self) -> Option<eyre::Result<DirItem>> {
        while let Some(dir) = self.stack.pop() {
            let read_dir =
                fs::read_dir(&dir).wrap_err_with(|| format!("couldn't read dir {}", dir.display()));

            match read_dir {
                Ok(entry) => {
                    self.current = entry;
                }
                Err(err) => return Some(Err(err)),
            };

            let next = self.read_next();

            if next.is_some() {
                return next;
            }
        }

        return None;
    }

    fn read(config: &Path) -> eyre::Result<Self> {
        let current = fs::read_dir(&config)
            .wrap_err_with(|| format!("couldn't read config dir {}", config.display()))?;

        Ok(Self {
            current,
            stack: Vec::new(),
        })
    }
}

impl Iterator for DirIter {
    type Item = eyre::Result<DirItem>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(res) = self.read_next() {
            return Some(res);
        }

        self.find_next_dir()
    }
}

#[derive(Debug, Clone)]
struct DirItem {
    path: PathBuf,
    item_type: ItemType,
}

impl DirItem {
    fn dir(path: PathBuf) -> Self {
        Self {
            path,
            item_type: ItemType::Dir,
        }
    }

    fn file(path: PathBuf) -> Self {
        Self {
            path,
            item_type: ItemType::File,
        }
    }

    fn symlink(path: PathBuf) -> Self {
        Self {
            path,
            item_type: ItemType::Symlink,
        }
    }
}

#[derive(Debug, Clone)]
enum ItemType {
    File,
    Symlink,
    Dir,
}

#[instrument]
pub fn apply(confirm: bool, dry_run: bool) -> eyre::Result<()> {
    let config = crate::config();

    let repo = &config.dirs.repository;

    let config_dir = repo.join("config");
    let xdg_config = dirs::config_dir().ok_or_eyre("coldn't find config dir")?;

    // sync config
    let iter = DirIter::read(&config_dir)?;

    for item in iter {
        let item = item?;

        let to = xdg_config.join(item.path.strip_prefix(&config_dir)?);

        match item.item_type {
            ItemType::File | ItemType::Symlink => {
                fs::copy(&item.path, &to)
                    .wrap_err_with(|| format!("couldn't copy {}", item.path.display()))?;

                info!("coping {} -> {}", item.path.display(), to.display());
            }
            ItemType::Dir => {
                let mode = item
                    .path
                    .metadata()
                    .wrap_err_with(|| format!("couldn't get dir metadata {}", item.path.display()))?
                    .mode();

                DirBuilder::new()
                    .recursive(true)
                    .mode(mode)
                    .create(&item.path)
                    .wrap_err_with(|| {
                        format!("couldn't create directory {}", item.path.display())
                    })?;

                info!("created direcotry {}", to.display());
            }
        }
    }

    // TODO: sync home

    Ok(())
}
