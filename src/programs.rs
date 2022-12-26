use std::{
    process::{Child, Command},
    sync::{Arc, Mutex},
    thread,
};

use crossbeam::channel::{select, unbounded, Receiver, Sender};

use crate::{
    errors::SupersError,
    messages::{CommandMsg, ProgramMsg},
    state::{ApplicationState, ProgramStatus},
    ProgramConfig,
};

// fn process_cmd_channel_msg(
//     msg: ProgramMsg,
//     threads_sx: &Sender<ProgramMsg>,
//     mut current_child: &Option<std::process::Child>,
// ) -> Result<(), SupersError> {
//     match msg {
//         // Start is a no-op
//         ProgramMsg::Start => {}
//         // For Stop, we need to send a Stop message on the threads channel and then stop the existing child
//         ProgramMsg::Stop => {
//             threads_sx.send(ProgramMsg::Stop);
//             match current_child {
//                 Some(mut c) => c.kill(),
//                 None => { return Ok(()); },
//             };
//         },
//         ProgramMsg::Restart => todo!(),
//         ProgramMsg::NewChild(_) => todo!(),
//     }
//     Ok(())
// }

/// Function to monitor and process the command channel for a specific program.
///
pub fn cmd_thread(
    cmd_rx: Receiver<CommandMsg>,
    pgms_rx: Receiver<ProgramMsg>,
    threads_sx: Sender<CommandMsg>,
) -> Result<(), SupersError> {
    let mut current_child: Option<Arc<Mutex<Child>>> = None;
    loop {
        select! {
            recv(cmd_rx) -> msg => {
                match msg {
                    // For Start, we send a Start message on the threads channel
                    Ok(CommandMsg::Start) => {
                        threads_sx.send(CommandMsg::Start);
                    }
                    // For Stop, we send a Stop message on the threads channel and kill the current child
                    // Stop message must be sent before killing child, as the programs thread is blocking on
                    // the child pocess exiting and it will expect the threads message to be waiting as soon as it exits.
                    Ok(CommandMsg::Stop) => {
                        threads_sx.send(CommandMsg::Stop);
                        match current_child {
                            Some(mut child) => {
                                let mut c = child.lock().unwrap().kill();
                                // *c.kill();
                                drop(c);
                                current_child = None;
                            },
                            None => {}
                        };
                    },
                    // For Restart, we send a Restart message on the threads channel and kill the current child
                    Ok(CommandMsg::Restart) => {
                        threads_sx.send(CommandMsg::Restart);
                        match current_child {
                            Some(mut c) => {
                                let _r = c.kill();
                                current_child = None;
                            }
                            None => {}
                        };
                    },
                    Err(e) => println!("Got error trying to receive msg from cmd channel; details: {:?}", e),
                };
            },
            recv(pgms_rx) -> msg => {
                match msg {
                    Ok(ProgramMsg::NewChild(c)) => current_child = Some(c),
                    Err(e) => {
                        println!("Got error trying to receive msg from pgms channel; details: {:?}", e);
                    }
                }

            }
        }
    }
    Ok(())
}

/// Function to start and monitor a program with ProgramConfig, `p`.
pub fn pgm_thread(
    p: ProgramConfig,
    app_state: Arc<Mutex<ApplicationState>>,
    pgms_sx: Sender<ProgramMsg>,
    threads_rx: Receiver<CommandMsg>,
) -> Result<(), SupersError> {
    loop {
        let program_name = p.name.clone();
        // wait for a Start message to start the program
        let msg = threads_rx
            .recv()
            .map_err(|e| SupersError::ProgramThreadThreadsChannelError(program_name, e))?;
        match msg {
            CommandMsg::Stop => {
                // Consume all accumulated stop messages while the program is stopped.
                continue;
            }
            CommandMsg::Start | CommandMsg::Restart => loop {
                // Start the program as a child process based on the config, `p`.
                let mut child = Command::new(&p.cmd)
                    .args(&p.args)
                    .envs(&p.env)
                    .spawn()
                    .map_err(|e| SupersError::ProgramProcessSpawnError(p.name.to_string(), e))?;
                // Update the program's status to Running in the app_state
                let mut a = app_state.lock().unwrap();
                *a.programs
                    .entry(p.name.to_string())
                    .or_insert(ProgramStatus::Running) = ProgramStatus::Running;
                drop(a);

                // Send the resulting child process to the cmd thread
                let cm = Arc::new(Mutex::new(child));
                pgms_sx.send(ProgramMsg::NewChild(cm));
                // Consume any Start messages that have accumulated while we were waiting to spawn the child process
                let exit_status = child
                    .wait()
                    .map_err(|e| SupersError::ProgramProcessExitError(program_name, e))?;
            },
        }
    }

    Ok(())
}

pub fn test_channels() -> () {
    let (s_pgm, r_pgm) = unbounded::<i32>();
    let (s_cmd, r_cmd) = unbounded::<i32>();
    let (s_threads, r_threads) = unbounded::<i32>();

    let START = 11;
    let STOP = 12;
    let RESTART = 13;

    let pgms_thread = thread::spawn(move || {
        // start program, get a child, send it over the programs channel ---
        let mut child = 1;
        loop {
            if child > 3 {
                break;
            }
            let _r = s_pgm.send(child);
            // would wait for child to exit..
            let msg = r_threads.recv().unwrap();
            println!(
                "pgrms_thread got a message on the threads channel: {:?}",
                msg
            );

            child += 1;
        }
    });

    let cmds_thread = thread::spawn(move || {
        let mut msg = 0;
        loop {
            select! {
                recv(r_pgm) -> msg => println!("cmds_thread got a message from the programs thread: {:?}", msg),
                recv(r_cmd) -> msg => {
                    println!("cmds_thread got a message from the command channel: {:?}", msg);
                    // for a START, just send the message
                    if msg == Ok(START) {
                        s_threads.send(START);
                    }
                    // if the command is a STOP or RESTART,
                    // need to send a stop message to thread 1 and then kill the child
                    if msg == Ok(STOP) {
                        s_threads.send(STOP);
                        // kill child ...
                    }
                    if msg == Ok(RESTART) {
                        s_threads.send(RESTART);
                        // kill child
                    }
                },
            }
            msg += 1;
            if msg > 5 {
                break;
            }
        }
    });
    // send some commands ---
    let _r = s_cmd.send(START);
    let _r = s_cmd.send(RESTART);
    let _r = s_cmd.send(STOP);
    let _r = pgms_thread.join();
    let _r = cmds_thread.join();

    ()
}

#[cfg(test)]
mod test {
    use super::test_channels;

    #[test]
    fn test() {
        test_channels();
    }
}
