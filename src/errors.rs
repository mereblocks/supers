use crossbeam::channel::SendError;
use thiserror::Error;

use crate::messages::CommandMsg;

/// SupersError enumerates all possible error types returned by supers.
#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum SupersError {
    #[error("supers was unable to parse application config; details: {0}")]
    ApplicationConfigParseError(toml::de::Error),

    #[error("supers was unable to read application config file; details: {0}")]
    ApplicationConfigFileError(std::io::Error),

    #[error("supers was unable to start thread for program {0}; details: {1}")]
    ProgramThreadStartError(String, std::io::Error),

    #[error("supers was unable to check process status for program {0}; details: {1}")]
    ProgramCheckProcessStatusError(String, std::io::Error),

    #[error("supers got error trying to recv message from command channel for program {0}; details: {1}")]
    ProgramCmdChannelError(String, std::io::Error),

    #[error(
        "supers failed to spawn child process for program {0}; details: {1}"
    )]
    ProgramProcessSpawnError(String, std::io::Error),

    #[error(
        "supers failed to collect exit status from child process for program {0}; details: {1}"
    )]
    ProgramProcessExitError(String, std::io::Error),

    #[error(
        "supers failed to kill child process for program {0}; details: {1}"
    )]
    ProgramProcessKillError(String, std::io::Error),

    #[error("supers got error while sending a command message")]
    ProgramCommandChannelSendError {
        #[from]
        source: SendError<CommandMsg>,
    },
}
