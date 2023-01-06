/// Messages sent on the command channel
#[derive(Debug, PartialEq, Eq)]
pub enum CommandMsg {
    Start,
    Stop,
    Restart,
}
