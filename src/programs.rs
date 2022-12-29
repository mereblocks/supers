use core::time;
use std::{
    collections::HashMap,
    process::{Child, Command},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use crossbeam::channel::{unbounded, Receiver, Sender};

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

/// Update the status of program with name, `pgm_name`, to status, `status`.
/// This function panics if it cannot lock the app_state object.
pub fn update_pgm_status(
    app_state: Arc<Mutex<ApplicationState>>,
    pgm_name: String,
    status: ProgramStatus,
) {
    let mut a = app_state.lock().unwrap();
    *a.programs.entry(pgm_name).or_insert(status) = status;
    drop(a);
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
        match current_child.as_mut() {
            Some(child) => {
                status = child.try_wait().map_err(|e| {
                    SupersError::ProgramCheckProcessStatusError(p.name.to_string(), e)
                })?;
                // If we got a status, set the program status to Stopped and set the
                // current_child to None (TODO -- check this)
                match status {
                    Some(_s) => {
                        let app_state_clone = app_state.clone();
                        update_pgm_status(
                            app_state_clone,
                            p.name.to_string(),
                            ProgramStatus::Stopped,
                        );

                        current_child = None;
                    }
                    None => {}
                };
            }
            None => {}
        };
        // check for a new message on the command channel
        let msg = cmd_rx.recv_timeout(WAIT_TIMEOUT);

        match current_child.as_mut() {
            // If current_child is None, the progam is not running; We first check if we have a new command msg to process.
            None => {
                match msg {
                    // Nothing to do for a Stop; the current program is not running
                    Ok(CommandMsg::Stop) => {}
                    // if we have Start or Restart message, we start a new child process
                    Ok(CommandMsg::Start) | Ok(CommandMsg::Restart) => {
                        current_child = Some(start_child_program(&p)?);
                        let app_state_clone = app_state.clone();
                        update_pgm_status(
                            app_state_clone,
                            p.name.to_string(),
                            ProgramStatus::Running,
                        );
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
                        else if p.restartpolicy == RestartPolicy::Always
                            || p.restartpolicy == RestartPolicy::OnError
                        {
                            restart_program = true;
                        };
                        // restart the program in a new child process
                        if restart_program {
                            current_child = Some(start_child_program(&p)?);
                            let app_state_clone = app_state.clone();
                            update_pgm_status(
                                app_state_clone,
                                p.name.to_string(),
                                ProgramStatus::Running,
                            );
                        };
                        // At this point, we have "processed" the status, so we set it back to None.
                        status = None;
                    }
                };
            }
            // If we do have a child, the program is running and so we only need to check for a command msg.
            // In particular, we do not have a status to check because that would imply we do not have a child.
            Some(c) => {
                match msg {
                    // A Stop command requires to kill the current child
                    Ok(CommandMsg::Stop) => {
                        let _r = c.kill();
                        // TODO -- handle error from attempt to kill child.
                        let app_state_clone = app_state.clone();
                        update_pgm_status(
                            app_state_clone,
                            p.name.to_string(),
                            ProgramStatus::Stopped,
                        );
                        current_child = None;
                    }
                    // On a Resart, we fist fill the running process and update the status to Stopped,
                    // Then we start a new process and update the status to Running.
                    // The net effect is that the status goes from Running -> Stopped -> Running, but if
                    // the attempt to start a new process fails, we will correctly leave the status in Stopped.
                    Ok(CommandMsg::Restart) => {
                        let _r = c.kill();
                        let app_state_clone = app_state.clone();
                        update_pgm_status(
                            app_state_clone,
                            p.name.to_string(),
                            ProgramStatus::Stopped,
                        );
                        // TODO -- handle error from attempt to kill child.
                        current_child = Some(start_child_program(&p)?);
                        let app_state_clone = app_state.clone();
                        update_pgm_status(
                            app_state_clone,
                            p.name.to_string(),
                            ProgramStatus::Running,
                        );
                    }
                    // Start is a no-op, as the program is already running
                    Ok(CommandMsg::Start) => {}

                    // Timeout error on a message is a no-op for now; TODO
                    Err(_) => {}
                };
            }
        };
    }
}

/// Main entrypoint for the programs.rs module; For each program in the app_config, this function:
/// 1) creates a command channel to process commands from the administrative API
/// 2) starts a thread to run and monitor the program, passing in the command channel.
pub fn start_program_threads(
    app_config: Vec<ProgramConfig>,
    app_state: &Arc<Mutex<ApplicationState>>,
) -> Result<(Vec<JoinHandle<()>>, HashMap<String, Sender<CommandMsg>>), SupersError> {
    let mut handles: Vec<JoinHandle<()>> = vec![];
    let mut send_channels = HashMap::new();
    // start a thread for each program in the config
    for program in app_config {
        let p = program.clone();
        let t = thread::Builder::new().name(program.name);
        let program_name = p.name.clone();
        let (tx, rx) = unbounded::<CommandMsg>();
        let app_state_clone = app_state.clone();
        send_channels.insert(program_name.to_string(), tx);
        let handle = t
            .spawn(move || {
                println!("Starting supers thread for program {}...", p.name);
                let _result = pgm_thread(p, app_state_clone, rx);
            })
            .map_err(|e| SupersError::ProgramThreadStartError(program_name, e))?;
        handles.push(handle);
    }

    Ok((handles, send_channels))
}

#[cfg(test)]
mod test {
    use std::thread;

    use crossbeam::channel::{select, unbounded};

    #[test]
    pub fn test_channels() {
        let (s_pgm, r_pgm) = unbounded::<i32>();
        let (s_cmd, r_cmd) = unbounded::<i32>();
        let (s_threads, r_threads) = unbounded::<i32>();

        let start = 11;
        let stop = 12;
        let restart = 13;

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
                        if msg == Ok(start) {
                            let _r = s_threads.send(start);
                        }
                        // if the command is a STOP or RESTART,
                        // need to send a stop message to thread 1 and then kill the child
                        if msg == Ok(stop) {
                            let _r =  s_threads.send(stop);
                            // kill child ...
                        }
                        if msg == Ok(restart) {
                            let _r =  s_threads.send(restart);
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
        let _r = s_cmd.send(start);
        let _r = s_cmd.send(restart);
        let _r = s_cmd.send(stop);
        let _r = pgms_thread.join();
        let _r = cmds_thread.join();
    }
}
