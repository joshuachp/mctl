use std::{
    io::{Write, stdout},
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
pub enum Secret {
    /// Edits a secret
    Edit {
        /// Allow a secret to be empty.
        #[arg(default_value = "false", long)]
        allow_empty: bool,
        /// Read the secret from stdin
        #[arg(default_value = "false", long)]
        stdin: bool,
        /// Path to the secret file
        file: PathBuf,
    },
    /// Cats a secret
    Cat {
        /// Path to the secret file
        file: PathBuf,
    },
    Rotate {
        /// Path to the secret file
        file: PathBuf,
    },
}

impl Secret {
    pub(crate) fn run(&self) -> eyre::Result<()> {
        match self {
            Secret::Edit {
                stdin,
                allow_empty,
                file,
            } => {
                if *stdin {
                    return mctl::secret::from_stdin(*allow_empty, file);
                }

                mctl::secret::edit(file, *allow_empty)
            }
            Secret::Cat { file } => mctl::secret::cat(file),
            Secret::Rotate { file } => mctl::secret::rotate(file),
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
        let shell = match self {
            Shell::Bash => clap_complete::Shell::Bash,
            Shell::Zsh => clap_complete::Shell::Zsh,
            Shell::Fish => clap_complete::Shell::Fish,
        };

        let mut stdout = stdout().lock();
        clap_complete::generate(
            shell,
            &mut Cli::command(),
            env!("CARGO_BIN_NAME"),
            &mut stdout,
        );
        stdout.flush()?;

        Ok(())
    }
}
