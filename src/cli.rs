use std::{
    io::{stdout, Write},
    path::PathBuf,
};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[clap(version, about, bin_name = env!("CARGO_BIN_NAME"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Utility functions like shell completions
    Utils {
        #[command(subcommand)]
        command: Utils,
    },
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
