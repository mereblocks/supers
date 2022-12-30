/// Messages sent on the command channel
#[derive(Debug)]
pub enum CommandMsg {
    Start,
    Stop,
    Restart,
}
