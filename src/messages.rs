/// Messages sent on the command channel
#[derive(Debug, PartialEq)]
pub enum CommandMsg {
    Start,
    Stop,
    Restart,
}
