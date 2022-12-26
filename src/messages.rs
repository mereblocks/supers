use std::{
    process::Child,
    sync::{Arc, Mutex},
};

/// Messages sent on the command channel
pub enum CommandMsg {
    Start,
    Stop,
    Restart,
}

/// Messages sent on the programs channel
pub enum ProgramMsg {
    NewChild(Arc<Mutex<Child>>),
}
