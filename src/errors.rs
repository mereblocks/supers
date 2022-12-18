

use thiserror::Error;


/// SupersError enumerates all possible error types returned by supers.
#[derive(Error, Debug)]
pub enum SupersError {
    
    #[error("supers base error")]
    BaseSupersError,

    #[error("supers was unable to start thread for program {0}; details: {1}")]
    ProgramThreadStartError(String, std::io::Error),

    #[error("supers failed to spawn child process for program {0}; details: {1}")]
    ProgramProcessSpawnError(String, std::io::Error),

    #[error("supers failed to collect exit status from child process for program {0}; details: {1}")]
    ProgramProcessExitError(String, std::io::Error),

}