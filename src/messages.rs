use std::process::Child;


#[derive(Debug)]
pub enum ProgramMsg {
    Start,
    Stop,
    Restart,
    NewChild(Child),
}

#[derive(Debug)]
pub enum ControlMsg {
    StopAll,
    StopControl,
}

