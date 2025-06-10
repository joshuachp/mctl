use std::sync::OnceLock;

use self::config::Config;

pub mod config;
pub mod secret;
pub(crate) mod util;

pub static CONFIG: OnceLock<Config> = OnceLock::new();

pub(crate) fn config() -> &'static Config {
    CONFIG.get().expect("the config must be initialized")
}
