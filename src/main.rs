use clap::Parser;
use cli::Cli;
use mctl::{config::Config, CONFIG};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod cli;

fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    color_eyre::install()?;
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env()?,
        )
        .try_init()?;

    let config = Config::read(cli.config)?;

    CONFIG.get_or_init(|| config);

    match cli.command {
        cli::Command::Sync { command } => {
            command.run()?;
        }
        cli::Command::Status { command: _ } => todo!(),
        cli::Command::Secret { command } => {
            command.run()?;
        }
        cli::Command::Utils { command } => {
            command.run()?;
        }
    }

    Ok(())
}
