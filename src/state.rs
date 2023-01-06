use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgramStatus {
    Running,
    Stopped,
}

impl Display for ProgramStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Default)]
pub enum ApplicationStatus {
    #[default]
    Running,
    // TODO -- uncomment when implementing the app stop endpoint
    // Stopped,
}

impl Display for ApplicationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Default)]
pub struct ApplicationState {
    pub application_status: ApplicationStatus,
    pub programs: HashMap<String, ProgramStatus>,
}
