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

/// Utilities and helpers for building the base node instance
mod builder;
/// The command line interface definition and configuration
mod cli;
/// Application-specific constants
mod consts;
/// Miner lib Todo hide behind feature flag
mod miner;
/// Parser module used to control user commands
mod parser;
mod utils;

use crate::builder::{create_new_base_node_identity, load_identity};
use log::*;
use parser::Parser;
use rustyline::{config::OutputStreamType, error::ReadlineError, CompletionType, Config, EditMode, Editor};
use std::{path::PathBuf, sync::Arc};
use tari_common::{load_configuration, GlobalConfig};
use tari_comms::{multiaddr::Multiaddr, peer_manager::PeerFeatures, NodeIdentity};
use tari_shutdown::Shutdown;
use tokio::runtime::Runtime;

pub const LOG_TARGET: &str = "base_node::app";

enum ExitCodes {
    ConfigError = 101,
    UnknownError = 102,
}

fn main() {
    cli::print_banner();
    match main_inner() {
        Ok(_) => std::process::exit(0),
        Err(exit_code) => std::process::exit(exit_code as i32),
    }
}

fn main_inner() -> Result<(), ExitCodes> {
    // Parse and validate command-line arguments
    let arguments = cli::parse_cli_args();

    // Initialise the logger
    if !tari_common::initialize_logging(&arguments.bootstrap.log_config) {
        return Err(ExitCodes::ConfigError);
    }

    // Load and apply configuration file
    let cfg = load_configuration(&arguments.bootstrap).map_err(|err| {
        error!(target: LOG_TARGET, "{}", err);
        ExitCodes::ConfigError
    })?;

    // Populate the configuration struct
    let node_config = GlobalConfig::convert_from(cfg).map_err(|err| {
        error!(target: LOG_TARGET, "The configuration file has an error. {}", err);
        ExitCodes::ConfigError
    })?;

    trace!(target: LOG_TARGET, "Using configuration: {:?}", node_config);

    // Set up the Tokio runtime
    let mut rt = setup_runtime(&node_config).map_err(|err| {
        error!(target: LOG_TARGET, "{}", err);
        ExitCodes::UnknownError
    })?;

    // Load or create the Node identity
    let wallet_identity = setup_node_identity(
        &node_config.wallet_identity_file,
        &node_config.public_address,
        arguments.create_id ||
            // If the base node identity exists, we want to be sure that the wallet identity exists
            node_config.identity_file.exists(),
        PeerFeatures::COMMUNICATION_CLIENT,
    )?;
    let node_identity = setup_node_identity(
        &node_config.identity_file,
        &node_config.public_address,
        arguments.create_id,
        PeerFeatures::COMMUNICATION_NODE,
    )?;

    // Build, node, build!
    let shutdown = Shutdown::new();
    let ctx = rt
        .block_on(builder::configure_and_initialize_node(
            &node_config,
            node_identity,
            wallet_identity,
            shutdown.to_signal(),
        ))
        .map_err(|err| {
            error!(target: LOG_TARGET, "{}", err);
            ExitCodes::UnknownError
        })?;

    // Exit if create_id or init arguments were run
    if arguments.create_id {
        info!(
            target: LOG_TARGET,
            "Node ID created at '{}'. Done.",
            node_config.identity_file.to_string_lossy()
        );
        return Ok(());
    }

    if arguments.init {
        info!(target: LOG_TARGET, "Default configuration created. Done.");
        return Ok(());
    }

    // Run, node, run!
    let parser = Parser::new(rt.handle().clone(), &ctx);
    let base_node_handle = rt.spawn(ctx.run(rt.handle().clone()));

    info!(
        target: LOG_TARGET,
        "Node has been successfully configured and initialized. Starting CLI loop."
    );

    cli_loop(parser, shutdown);

    match rt.block_on(base_node_handle) {
        Ok(_) => info!(target: LOG_TARGET, "Node shutdown successfully."),
        Err(e) => error!(target: LOG_TARGET, "Node has crashed: {}", e),
    }

    println!("Goodbye!");
    Ok(())
}

fn setup_runtime(config: &GlobalConfig) -> Result<Runtime, String> {
    let num_core_threads = config.core_threads;
    let num_blocking_threads = config.blocking_threads;
    let num_mining_threads = config.num_mining_threads;

    debug!(
        target: LOG_TARGET,
        "Configuring the node to run on {} core threads, {} blocking worker threads and {} mining threads.",
        num_core_threads,
        num_blocking_threads,
        num_mining_threads
    );
    tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .max_threads(num_core_threads + num_blocking_threads + num_mining_threads)
        .core_threads(num_core_threads)
        .build()
        .map_err(|e| format!("There was an error while building the node runtime. {}", e.to_string()))
}

fn cli_loop(parser: Parser, mut shutdown: Shutdown) {
    let cli_config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .output_stream(OutputStreamType::Stdout)
        .build();
    let mut rustyline = Editor::with_config(cli_config);
    rustyline.set_helper(Some(parser));
    loop {
        let readline = rustyline.readline(">> ");
        match readline {
            Ok(line) => {
                rustyline.add_history_entry(line.as_str());
                if let Some(p) = rustyline.helper_mut().as_deref_mut() {
                    p.handle_command(&line, &mut shutdown)
                }
            },
            Err(ReadlineError::Interrupted) => {
                // shutdown section. Will shutdown all interfaces when ctrl-c was pressed
                println!("The node is shutting down because Ctrl+C was received...");
                info!(
                    target: LOG_TARGET,
                    "Termination signal received from user. Shutting node down."
                );
                if shutdown.trigger().is_err() {
                    error!(target: LOG_TARGET, "Shutdown signal failed to trigger");
                };
                break;
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            },
        }
        if shutdown.is_triggered() {
            break;
        };
    }
}

fn setup_node_identity(
    identity_file: &PathBuf,
    public_address: &Multiaddr,
    create_id: bool,
    peer_features: PeerFeatures,
) -> Result<Arc<NodeIdentity>, ExitCodes>
{
    match load_identity(identity_file) {
        Ok(id) => Ok(Arc::new(id)),
        Err(e) => {
            if !create_id {
                error!(
                    target: LOG_TARGET,
                    "Node identity information not found. {}. You can update the configuration file to point to a \
                     valid node identity file, or re-run the node with the --create_id flag to create a new identity.",
                    e
                );
                return Err(ExitCodes::ConfigError);
            }

            debug!(target: LOG_TARGET, "Node id not found. {}. Creating new ID", e);

            match create_new_base_node_identity(identity_file, public_address.clone(), peer_features) {
                Ok(id) => {
                    info!(
                        target: LOG_TARGET,
                        "New node identity [{}] with public key {} has been created at {}.",
                        id.node_id(),
                        id.public_key(),
                        identity_file.to_string_lossy(),
                    );
                    Ok(Arc::new(id))
                },
                Err(e) => {
                    error!(target: LOG_TARGET, "Could not create new node id. {:?}.", e);
                    Err(ExitCodes::ConfigError)
                },
            }
        },
    }
}
