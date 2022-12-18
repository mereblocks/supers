use std::{collections::HashMap, fmt::Display};

#[derive(Debug)]
pub enum ProgramStatus {
    Running,
    Stopped,
}

impl Display for ProgramStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}


#[derive(Debug)]
pub enum ApplicationStatus {
    Running,
    Stopped,
}

impl Display for ApplicationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct ApplicationState {
    
    pub application_status: ApplicationStatus,

    pub programs: HashMap<String, ProgramStatus>,

}