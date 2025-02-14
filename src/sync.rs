use std::{
    fmt::Display,
    fs::{self, DirBuilder, DirEntry, ReadDir},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};

use eyre::{OptionExt, WrapErr};
use tracing::{info, instrument, warn};

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

    fn copy(&self, base_dir: &Path, target_dir: &Path) -> eyre::Result<()> {
        let rel_path = self.path.strip_prefix(&base_dir)?;
        let to = target_dir.join(rel_path);

        match self.item_type {
            ItemType::File | ItemType::Symlink => {
                fs::copy(&self.path, &to).wrap_err("couldn't copy file")?;

                info!("coping {} -> {}", self.path.display(), to.display());
            }
            ItemType::Dir => {
                let mode = self
                    .path
                    .metadata()
                    .wrap_err("couldn't get dir metadata")?
                    .mode();

                DirBuilder::new()
                    .recursive(true)
                    .mode(mode)
                    .create(&self.path)
                    .wrap_err("couldn't create directory")?;

                info!("created direcotry {}", to.display());
            }
        }

        Ok(())
    }
}

impl Display for DirItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.item_type, self.path.display())
    }
}

#[derive(Debug, Clone)]
enum ItemType {
    File,
    Symlink,
    Dir,
}

impl Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemType::File => write!(f, "file"),
            ItemType::Symlink => write!(f, "symlink"),
            ItemType::Dir => write!(f, "directory"),
        }
    }
}

#[instrument]
pub fn apply(confirm: bool, dry_run: bool) -> eyre::Result<()> {
    let config = crate::config();

    let repo = &config.dirs.repository;

    let repo_config_dir = repo.join("config");
    let xdg_config = dirs::config_dir().ok_or_eyre("coldn't find config dir")?;

    // sync config
    let iter = DirIter::read(&repo_config_dir)?;

    for item in iter {
        let item = item?;

        item.copy(&repo_config_dir, &xdg_config)
            .wrap_err_with(move || format!("couldn't copy {item}"))?;
    }

    // sync home
    let repo_home_dir = repo.join("home");
    let home = dirs::home_dir().ok_or_eyre("coldn't find config dir")?;

    let iter = DirIter::read(&repo_home_dir)?;

    for item in iter {
        let item = item?;

        item.copy(&repo_home_dir, &home)
            .wrap_err_with(move || format!("couldn't copy {item}"))?;
    }

    Ok(())
}
