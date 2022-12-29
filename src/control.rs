use crate::messages::{ControlMsg, ProgramMsg};
use anyhow::{Error, Result};
use crossbeam::channel::{Receiver, Sender};
use log::warn;
use std::thread::{self, JoinHandle};
use tracing::{event, span, Level};

pub fn start_control(
    control: &Receiver<ControlMsg>,
    programs: &[&Sender<ProgramMsg>],
) -> JoinHandle<Result<(), Error>> {
    let _span = span!(Level::INFO, "control").entered();
    event!(Level::WARN, "Starting control");
    let control = control.clone();
    let programs = programs.iter().cloned().cloned().collect::<Vec<_>>();
    thread::spawn(move || -> Result<()> {
        let _span = span!(Level::INFO, "control_thread").entered();
        loop {
            let msg = control.recv()?;
            event!(Level::INFO, msg = ?msg, "Got a message");
            match msg {
                ControlMsg::StopAll => {
                    for program in &programs {
                        program.send(ProgramMsg::Stop).unwrap_or_else(|_err| {
                            warn!("channel {:?} closed, ignoring", program);
                        })
                    }
                }
                ControlMsg::StopControl => {
                    warn!("Control thread shutting down.");
                    break;
                }
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use anyhow::Result;
    use crossbeam::channel::{unbounded, Receiver, Sender};
    use log::{info, LevelFilter};

    use crate::messages::{ControlMsg, ProgramMsg};

    use super::start_control;

    fn init_log() {
        let mut builder = env_logger::Builder::new();
        builder.filter_level(LevelFilter::Debug);
        builder.parse_default_env();
        builder.init();

        let x = tracing_subscriber::FmtSubscriber::new();
        tracing::subscriber::set_global_default(x).expect("setting default subscriber failed");
    }

    fn my_thread(
        name: &str,
        control: &Sender<ControlMsg>,
        command: &Receiver<ProgramMsg>,
    ) -> Result<()> {
        loop {
            info!("{name}: I'm thread {name}");
            let cmd = command.recv()?;
            info!("{name}: Was sent command {:?}", cmd);
            match cmd {
                ProgramMsg::Stop => {
                    info!("{name}: Was asked to stop...");
                    if name == "foo" {
                        control.send(ControlMsg::StopAll)?;
                        info!("{name}:  I'm foo, so asking everyone to stop");
                    }
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[test]
    fn test_foo() -> Result<()> {
        init_log();

        info!("Starting!");
        let (s1, r1) = unbounded();
        let (s2, r2) = unbounded();
        let (s3, r3) = unbounded();
        let (control_s, control_r) = unbounded();

        let t1;
        let t2;
        let t3;
        let control;
        {
            let control_s = control_s.clone();
            t1 = thread::spawn(move || my_thread("foo", &control_s, &r1));
        }
        {
            let control_s = control_s.clone();
            t2 = thread::spawn(move || my_thread("bar", &control_s, &r2));
        }
        {
            let control_s = control_s.clone();
            t3 = thread::spawn(move || my_thread("baz", &control_s, &r3));
        }
        {
            let s1 = s1.clone();
            let s2 = s2.clone();
            control = start_control(&control_r, &[&s1, &s2, &s3]);
        }
        info!("Control started");

        thread::sleep(Duration::from_secs(3));
        info!("Sending STOP to thread 'bar'");
        s2.send(ProgramMsg::Stop)?;

        thread::sleep(Duration::from_secs(3));
        info!("Sending STOP to thread 'foo'. All should stop!");
        s1.send(ProgramMsg::Stop)?;

        thread::sleep(Duration::from_secs(5));
        info!("Sending STOP to Control thread.");
        control_s.send(ControlMsg::StopControl)?;

        t1.join().expect("")?;
        t2.join().expect("")?;
        t3.join().expect("")?;
        control.join().expect("")?;
        Ok(())
    }
}
