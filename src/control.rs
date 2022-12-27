use crate::messages::{ControlMsg, ProgramMsg};
use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};
use std::thread;

pub fn start_control(
    control: &Receiver<ControlMsg>,
    programs: &[&Sender<ProgramMsg>],
) -> Result<()> {
    {
        let control = control.clone();
        let programs = programs.iter().cloned().cloned().collect::<Vec<_>>();
        let control_thread = thread::spawn(move || -> Result<()> {
            loop {
                let msg = control.recv()?;
                match msg {
                    ControlMsg::StopAll => {
                        for program in &programs {
                            program.send(ProgramMsg::Stop).unwrap_or_else(|_err| {
                                println!("channel {:?} closed, ignoring", program);
                            })
                        }
                    }
                    ControlMsg::StopControl => break,
                }
            }
            Ok(())
        });
        control_thread.join().expect("control thread panicked")?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use anyhow::Result;
    use crossbeam::channel::{unbounded, Receiver, Sender};

    use crate::messages::{ControlMsg, ProgramMsg};

    use super::start_control;

    fn my_thread(
        name: &str,
        control: &Sender<ControlMsg>,
        command: &Receiver<ProgramMsg>,
    ) -> Result<()> {
        loop {
            println!("{name}: I'm thread {name}");
            let cmd = command.recv()?;
            println!("{name}: Was sent command {:?}", cmd);
            match cmd {
                ProgramMsg::Stop => {
                    println!("{name}: Was asked to stop...");
                    if name == "foo" {
                        control.send(ControlMsg::StopAll)?;
                        println!("{name}:  I'm foo, so asking everyone to stop");
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
        let (s1, r1) = unbounded();
        let (s2, r2) = unbounded();
        let (s3, r3) = unbounded();
        let (control_s, control_r) = unbounded();

        let t1;
        let t2;
        let t3;
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
            thread::spawn(move || -> Result<()> {
                start_control(&control_r, &[&s1, &s2, &s3])?;
                Ok(())
            });
        }
        println!("Control started");

        thread::sleep(Duration::from_secs(3));
        println!("Sending STOP to thread 'bar'");
        s2.send(ProgramMsg::Stop)?;

        thread::sleep(Duration::from_secs(3));
        println!("Sending STOP to thread 'foo'. All should stop!");
        s1.send(ProgramMsg::Stop)?;

        t1.join().expect("")?;
        t2.join().expect("")?;
        t3.join().expect("")?;
        Ok(())
    }
}
