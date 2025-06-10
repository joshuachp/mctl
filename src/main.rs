use clap::Parser;
use cli::{Cli, Command};
use mctl::{CONFIG, config::Config};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

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

    // Don't require a config for utils
    if let Command::Utils { command } = cli.command {
        return command.run();
    }

    let config = Config::read(cli.config.as_deref())?;

    CONFIG.get_or_init(|| config);

    match cli.command {
        Command::Secret { command } => {
            command.run()?;
        }
        Command::Utils { .. } => {}
    }

    Ok(())
}
