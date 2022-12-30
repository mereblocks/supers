use serde_derive::Deserialize;
use std::collections::HashMap;
use std::env;
use std::path::Path;

use crate::errors::SupersError;

/// These are the available restart policies for programs
#[derive(Clone, PartialEq, Debug, Deserialize)]
pub enum RestartPolicy {
    /// Always restart the program after it exits, regardless of exit status
    Always,

    /// Never restart the program, regardless of exist status
    Never,

    /// Restart the program if it exited with a non-success status, otherwise, do not restart
    OnError,
}

/// Configuration for a program to be launched and supervised by supers.
#[derive(Clone, Deserialize)]
pub struct ProgramConfig {
    /// The name of the program, used for naming the thread, logging, etc. Should be unique within a supers application
    pub name: String,

    /// The command used to start the program
    pub cmd: String,

    /// An array of arguments to the program's command.
    pub args: Vec<String>,

    /// The environment variables to set before starting the program, as key-value pairs
    pub env: HashMap<String, String>,

    /// The RestartPolicy for the program
    pub restartpolicy: RestartPolicy,
}

/// Configuration for the application iteself
#[derive(Deserialize)]
pub struct ApplicationConfig {
    /// The name of the application
    pub app_name: String,
    /// The programs comprising the application
    pub programs: Vec<ProgramConfig>,
}

pub fn get_app_config_from_str(s: String) -> Result<ApplicationConfig, SupersError> {
    let config: ApplicationConfig =
        toml::from_str(&s).map_err(|e| SupersError::ApplicationConfigParseError(e))?;
    Ok(config)
}

const DEFAULT_CONFIG_PATH: &str = "/etc/supers/conf.toml";

pub fn get_app_config_from_file() -> Result<ApplicationConfig, SupersError> {
    let config_file_input = env::var("SUPERS_CONF_FILE").unwrap_or(DEFAULT_CONFIG_PATH.to_string());
    let config_path = Path::new(&config_file_input);

    // read the config file from the path
    let config_file_text = std::fs::read_to_string(config_path)
        .map_err(|e| SupersError::ApplicationConfigFileError(e))?;

    // convert to config object
    let config = get_app_config_from_str(config_file_text)?;

    Ok(config)
}
