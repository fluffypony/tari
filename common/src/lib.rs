// Copyright 2019. The Tari Project
//
// Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
// following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following
// disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
// following disclaimer in the documentation and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote
// products derived from this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! # Common logging and configuration utilities
//!
//! ## The global Tari configuration file
//!
//! A single configuration file (usually `~/.tari/config.toml` is used to manage settings for all Tari applications
//! and nodes running on a single system, whether it's a base node, validator node, or wallet.
//!
//! Setting of configuration parameters is applied using the following order of precedence:
//!
//! 1. Command-line argument
//! 2. Environment variable
//! 3. `config.toml` file value
//! 4. Configuration default
//!
//! The utilities exposed in this crate are opinionated, but flexible. In general, all data is stored in a `.tari`
//! folder under your home folder.
//!
//! ### Example - Loading and deserializing the global config file
//!
//! ```edition2018
//! # use tari_common::*;
//! let bootstrap: ConfigBootstrap = ConfigBootstrap::default();
//! let config = default_config(&bootstrap);
//! let config = GlobalConfig::convert_from(config).unwrap();
//! assert_eq!(config.network, Network::MainNet);
//! assert_eq!(config.blocking_threads, 4);
//! ```

use clap::ArgMatches;
use std::path::{Path, PathBuf};

mod configuration;
#[macro_use]
mod logging;

pub mod protobuf_build;

pub mod dir_utils;
pub use configuration::{
    default_config,
    install_default_config_file,
    load_configuration,
    CommsTransport,
    ConfigExtractor,
    ConfigurationError,
    DatabaseType,
    GlobalConfig,
    Network,
    SocksAuthentication,
    TorControlAuthentication,
};
pub use logging::initialize_logging;
use std::io;
pub const DEFAULT_CONFIG: &str = "config.toml";
pub const DEFAULT_LOG_CONFIG: &str = "log4rs.yml";

/// A minimal parsed configuration object that's used to bootstrap the main Configuration.
pub struct ConfigBootstrap {
    pub base_path: PathBuf,
    pub config: PathBuf,
    /// The path to the log configuration file. It is set using the following precedence set:
    ///   1. from the command-line parameter,
    ///   2. from the `TARI_LOG_CONFIGURATION` environment variable,
    ///   3. from a default value, usually `~/.tari/log4rs.yml` (or OS equivalent).
    pub log_config: PathBuf,
}

impl Default for ConfigBootstrap {
    fn default() -> Self {
        ConfigBootstrap {
            base_path: dir_utils::default_path("", None),
            config: dir_utils::default_path(DEFAULT_CONFIG, None),
            log_config: dir_utils::default_path(DEFAULT_LOG_CONFIG, None),
        }
    }
}

pub fn bootstrap_config_from_cli(matches: &ArgMatches) -> ConfigBootstrap {
    let base_path = matches
        .value_of("base_dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir_utils::default_path("", None));

    // Create the tari data directory
    if let Err(e) = dir_utils::create_data_directory(Some(&base_path)) {
        println!(
            "We couldn't create a default Tari data directory and have to quit now. This makes us sad :(\n {}",
            e.to_string()
        );
        std::process::exit(1);
    }

    let config = matches
        .value_of("config")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir_utils::default_path(DEFAULT_CONFIG, Some(&base_path)));

    let log_config = matches
        .value_of("log_config")
        .map(PathBuf::from)
        .or_else(|| Some(base_path.clone().join(DEFAULT_LOG_CONFIG)));
    let log_config = logging::get_log_configuration_path(log_config);

    if !config.exists() {
        let install = if !matches.is_present("init") {
            prompt("Config file does not exist. We can create a default one for you now, or you can say 'no' here, \
            and generate a customised one at https://config.tari.com.\n\
            Would you like to try the default configuration (Y/n)?")
        } else {
            true
        };

        if install {
            println!("Installing new config file at {}", config.to_str().unwrap_or("[??]"));
            install_configuration(&config, configuration::install_default_config_file);
        }
    }

    if !log_config.exists() {
        let install = if !matches.is_present("init") {
            prompt("Logging configuration file does not exist. Would you like to create a new one (Y/n)?")
        } else {
            true
        };
        if install {
            println!(
                "Installing new logfile configuration at {}",
                log_config.to_str().unwrap_or("[??]")
            );
            install_configuration(&log_config, logging::install_default_logfile_config);
        }
    }
    ConfigBootstrap {
        base_path,
        config,
        log_config,
    }
}

fn prompt(question: &str) -> bool {
    println!("{}", question);
    let mut input = "".to_string();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_lowercase();
    input == "y" || input.is_empty()
}

pub fn install_configuration<F>(path: &Path, installer: F)
where F: Fn(&Path) -> Result<(), std::io::Error> {
    if let Err(e) = installer(path) {
        println!(
            "We could not install a new configuration file in {}: {}",
            path.to_str().unwrap_or("?"),
            e.to_string()
        )
    }
}

#[cfg(test)]
mod test {
    use crate::{bootstrap_config_from_cli, dir_utils, dir_utils::default_subdir, load_configuration};
    use clap::clap_app;
    use tari_test_utils::random::string;
    use tempdir::TempDir;

    #[test]
    fn test_bootstrap_config_from_cli_and_load_configuration() {
        let temp_dir = TempDir::new(string(8).as_str()).unwrap();
        let dir = &temp_dir.path().to_path_buf();
        // Create test folder
        dir_utils::create_data_directory(Some(dir)).unwrap();

        // Create command line test data
        let matches = clap_app!(myapp =>
            (version: "0.0.10")
            (author: "The Tari Community")
            (about: "The reference Tari cryptocurrency base node implementation")
            (@arg base_dir: -b --base_dir +takes_value "A path to a directory to store your files")
            (@arg config: -c --config +takes_value "A path to the configuration file to use (config.toml)")
            (@arg log_config: -l --log_config +takes_value "A path to the logfile configuration (log4rs.yml))")
            (@arg init: --init "Create a default configuration file if it doesn't exist")
            (@arg create_id: --create_id "Create and save new node identity if one doesn't exist ")
        )
        .get_matches_from(vec![
            "",
            "--base_dir",
            default_subdir("", Some(dir)).as_str(),
            "--init",
            "--create_id",
        ]);

        // Load bootstrap
        let bootstrap = bootstrap_config_from_cli(&matches);
        let config_exists = std::path::Path::new(&bootstrap.config).exists();
        let log_config_exists = std::path::Path::new(&bootstrap.log_config).exists();
        // Load and apply configuration file
        let cfg = load_configuration(&bootstrap);

        // Cleanup test data
        if std::path::Path::new(&dir_utils::default_subdir("", Some(dir))).exists() {
            std::fs::remove_dir_all(&dir_utils::default_subdir("", Some(dir))).unwrap();
        }

        // Assert results
        assert!(config_exists);
        assert!(log_config_exists);
        assert!(&cfg.is_ok());
    }

    #[test]
    fn check_homedir_is_used_by_default() {
        dir_utils::create_data_directory(None).unwrap();
        assert_eq!(
            dirs::home_dir().unwrap().join(".tari"),
            dir_utils::default_path("", None)
        );
    }
}
