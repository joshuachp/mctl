use std::{
    io::{stdout, Write},
    path::PathBuf,
};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[clap(version, about, bin_name = env!("CARGO_BIN_NAME"))]
pub struct Cli {
    /// Additional configuration file to load.
    #[arg(long, short)]
    pub(crate) config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Updates the files in the home directory.
    Sync {
        #[command(subcommand)]
        command: SyncCmd,
    },
    /// Checks the status of the repo with the home
    Status {
        #[command(subcommand)]
        command: Status,
    },
    /// Manages the secrets
    Secret {
        #[command(subcommand)]
        command: Secret,
    },
    /// Utility functions like shell completions
    Utils {
        #[command(subcommand)]
        command: Utils,
    },
}
#[derive(Debug, Subcommand)]
pub enum SyncCmd {
    /// Syncs all the files in the repo to the home directory.
    Apply {
        /// Automatically sync all the files.
        #[arg(long, short = 'y', default_value = "false")]
        confirm: bool,
        /// Automatically sync all the files.
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },
}
impl SyncCmd {
    pub(crate) fn run(&self) -> eyre::Result<()> {
        match self {
            SyncCmd::Apply { confirm, dry_run } => mctl::sync::apply(*confirm, *dry_run),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Status {}

#[derive(Debug, Subcommand)]
pub enum Secret {
    /// Edits a secret
    Edit {
        /// Allow a secret to be empty.
        #[arg(default_value = "false", long)]
        allow_empty: bool,

        /// Path to the secret file
        file: PathBuf,
    },
}

impl Secret {
    pub(crate) fn run(&self) -> eyre::Result<()> {
        match self {
            Secret::Edit { file, allow_empty } => {
                if file.as_os_str() == "-" {
                    return mctl::secret::from_stdin();
                }

                mctl::secret::edit(file, *allow_empty)
            }
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Utils {
    /// Generates shell completions for the given shell
    Completion {
        /// Shell to generate the completion for
        shell: Shell,
    },
    /// Generates the Man pages for the program
    Manpages {
        /// Directory to generate the Man pages to.
        out_dir: PathBuf,
    },
}

impl Utils {
    pub(crate) fn run(&self) -> eyre::Result<()> {
        match self {
            Utils::Completion { shell } => shell.generate(),
            Utils::Manpages { out_dir } => {
                clap_mangen::generate_to(Cli::command(), out_dir)?;

                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    fn generate(&self) -> eyre::Result<()> {
        let gen = match self {
            Shell::Bash => clap_complete::Shell::Bash,
            Shell::Zsh => clap_complete::Shell::Zsh,
            Shell::Fish => clap_complete::Shell::Fish,
        };

        let mut stdout = stdout().lock();
        clap_complete::generate(
            gen,
            &mut Cli::command(),
            env!("CARGO_BIN_NAME"),
            &mut stdout,
        );
        stdout.flush()?;

        Ok(())
    }
}
