use core::time;
use std::{
    process::{Child, Command, ExitStatus},
    sync::{Arc, Mutex},
    thread,
};

use crossbeam::channel::{select, unbounded, Receiver};

use crate::{
    errors::SupersError,
    messages::CommandMsg,
    state::{ApplicationState, ProgramStatus},
    ProgramConfig, RestartPolicy,
};

// Amount of time the command thread will wait for a command message on the command channel.
pub static WAIT_TIMEOUT: time::Duration = time::Duration::from_millis(10);

/// Function to start a program with config given by, `p`, in a child process.
pub fn start_child_program(p: &ProgramConfig) -> Result<Child, SupersError> {
    let child = Command::new(&p.cmd)
        .args(&p.args)
        .envs(&p.env)
        .spawn()
        .map_err(|e| SupersError::ProgramProcessSpawnError(p.name.to_string(), e))?;

    Ok(child)
}

pub fn check_pgm_status(
    child: &mut Option<Child>,
    pgm_name: String,
) -> Result<Option<ExitStatus>, SupersError> {
    match *child {
        Some(ref child) => {
            return Ok(child.try_wait().map_err(|e| {
                SupersError::ProgramCheckProcessStatusError(pgm_name.to_string(), e)
            }))?;
        }
        None => return Ok(None),
    };
}

/// Function to start and monitor a process while also monitoring and processing the
/// associated command channel for a specific program.
///
pub fn pgm_thread(
    p: ProgramConfig,
    app_state: Arc<Mutex<ApplicationState>>,
    cmd_rx: Receiver<CommandMsg>,
) -> Result<(), SupersError> {
    let mut current_child: Option<Child> = None;
    let mut status = None;
    loop {
        // First, check the status of the current child if we have one.
        // status = check_pgm_status(current_child, p.name.to_string())?;
        match current_child {
            Some(mut child) => {
                status = child.try_wait().map_err(|e| {
                    SupersError::ProgramCheckProcessStatusError(p.name.to_string(), e)
                })?;
                // Set current_child to None if status is Some; TODO -- check this
                match status {
                    Some(_s) => current_child = None,
                    None => {}
                };
            }
            None => {}
        };
        // check for a new message on the command channel
        let msg = cmd_rx.recv_timeout(WAIT_TIMEOUT);

        match current_child {
            // If current_child is None, the progam is not running; We first check if we have a new command msg to process.
            None => {
                match msg {
                    // Nothing to do for a Stop; the current program is not running
                    Ok(CommandMsg::Stop) => {}
                    // if we have Start or Restart message, we start a new child process
                    Ok(CommandMsg::Start) | Ok(CommandMsg::Restart) => {
                        current_child = Some(start_child_program(&p)?);
                        let mut a = app_state.lock().unwrap();
                        *a.programs
                            .entry(p.name.to_string())
                            .or_insert(ProgramStatus::Running) = ProgramStatus::Running;
                        drop(a);
                    }
                    // The error case is a timeout waiting for a message, we consider this a no-op for now. TODO.
                    Err(_) => {}
                };
                // Next, we match on the status; if the status is Some, then the program has just exited, so we need
                // to apply the restart policy.
                match status {
                    // If we don't have a status, then there is nothing to do. The program stopped in a prior loop iteration.
                    None => {}
                    // if we have a status, the program has just stopped; apply restart policy.
                    Some(s) => {
                        let mut restart_program = false;
                        // the program exited successfully so we only restart if the policy is Always
                        if s.success() {
                            if p.restartpolicy == RestartPolicy::Always {
                                restart_program = true;
                            };
                        }
                        // the program exited with an error, so we restart if the policy is Alwys or OnError
                        else {
                            if p.restartpolicy == RestartPolicy::Always
                                || p.restartpolicy == RestartPolicy::OnError
                            {
                                restart_program = true;
                            }
                        };
                        // restart the program in a new child process
                        if restart_program {
                            current_child = Some(start_child_program(&p)?);
                            let mut a = app_state.lock().unwrap();
                            *a.programs
                                .entry(p.name.to_string())
                                .or_insert(ProgramStatus::Running) = ProgramStatus::Running;
                            drop(a);
                        };
                    }
                };
            }
            // If we do have a child, the program is running and so we only need to check for a command msg.
            Some(mut c) => {
                match msg {
                    // A Stop command requires to kill the current child
                    Ok(CommandMsg::Stop) => {
                        c.kill();
                        current_child = None;
                    }
                    Ok(CommandMsg::Restart) => {
                        c.kill();
                        current_child = Some(start_child_program(&p)?);
                    }
                    // Start is a no-op, as the program is already running
                    Ok(CommandMsg::Start) => {}

                    // Timeout error on a message is a no-op for now; TODO
                    Err(_) => {}
                };
            }
        };

        // select! {
        //     recv(cmd_rx) -> msg => {
        //         match msg {
        //             // For Start, we send a Start message on the threads channel
        //             Ok(CommandMsg::Start) => {
        //                 threads_sx.send(CommandMsg::Start);
        //             }
        //             // For Stop, we send a Stop message on the threads channel and kill the current child
        //             // Stop message must be sent before killing child, as the programs thread is blocking on
        //             // the child pocess exiting and it will expect the threads message to be waiting as soon as it exits.
        //             Ok(CommandMsg::Stop) => {
        //                 threads_sx.send(CommandMsg::Stop);
        //                 match current_child {
        //                     Some(mut child) => {
        //                         let mut c = child.lock().unwrap().kill();
        //                         // *c.kill();
        //                         drop(c);
        //                         current_child = None;
        //                     },
        //                     None => {}
        //                 };
        //             },
        //             // For Restart, we send a Restart message on the threads channel and kill the current child
        //             Ok(CommandMsg::Restart) => {
        //                 threads_sx.send(CommandMsg::Restart);
        //                 match current_child {
        //                     Some(mut child) => {
        //                         let mut c = child.lock().unwrap().kill();
        //                         // *c.kill();
        //                         drop(c);
        //                         current_child = None;
        //                     },
        //                     None => {}
        //                 };
        //             },
        //             Err(e) => println!("Got error trying to receive msg from cmd channel; details: {:?}", e),
        //         };
        //     },
        //     recv(pgms_rx) -> msg => {
        //         match msg {
        //             Ok(ProgramMsg::NewChild(c)) => current_child = Some(c),
        //             Err(e) => {
        //                 println!("Got error trying to receive msg from pgms channel; details: {:?}", e);
        //             }
        //         }

        //     }
        // }
    }
    // Ok(())
}

/// Function to start and monitor a program with ProgramConfig, `p`.
// pub fn pgm_thread(
//     p: ProgramConfig,
//     app_state: Arc<Mutex<ApplicationState>>,
//     pgms_sx: Sender<ProgramMsg>,
//     threads_rx: Receiver<CommandMsg>,
// ) -> Result<(), SupersError> {
//     loop {
//         let program_name = p.name.clone();
//         // wait for a Start message to start the program
//         let msg = threads_rx
//             .recv()
//             .map_err(|e| SupersError::ProgramThreadThreadsChannelError(program_name, e))?;
//         match msg {
//             CommandMsg::Stop => {
//                 // Consume all accumulated stop messages while the program is stopped.
//                 continue;
//             }
//             CommandMsg::Start | CommandMsg::Restart => loop {
//                 // Start the program as a child process based on the config, `p`.
//                 let mut child = Command::new(&p.cmd)
//                     .args(&p.args)
//                     .envs(&p.env)
//                     .spawn()
//                     .map_err(|e| SupersError::ProgramProcessSpawnError(p.name.to_string(), e))?;
//                 // Update the program's status to Running in the app_state
//                 let mut a = app_state.lock().unwrap();
//                 *a.programs
//                     .entry(p.name.to_string())
//                     .or_insert(ProgramStatus::Running) = ProgramStatus::Running;
//                 drop(a);

//                 // Send the resulting child process to the cmd thread
//                 let cm = Arc::new(Mutex::new(child)).clone();
//                 pgms_sx.send(ProgramMsg::NewChild(cm));
//                 // Consume any Start messages that have accumulated while we were waiting to spawn the child process
//                 let exit_status = child
//                     .wait()
//                     .map_err(|e| SupersError::ProgramProcessExitError(program_name, e))?;
//             },
//         }
//     }

//     Ok(())
// }

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
