use core::time;
use std::collections::HashMap;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep, JoinHandle};

use actix_web::web::Data;
use actix_web::{App, HttpServer};

use errors::SupersError;
use state::{ApplicationState, ApplicationStatus};

use crate::state::ProgramStatus;

mod errors;
mod handlers;
mod state;
mod programs;

/// These are the available restart policies for programs
#[derive(Clone, PartialEq)]
pub enum RestartPolicy {
    // Always restart the program after it exits, regardless of exit status
    Always,

    // Never restart the program, regardless of exist status
    Never,

    // Restart the program if it exited with a non-success status, otherwise, do not restart
    OnError,
}

/// Configuration for a program to be launched and supervised by supers.
#[derive(Clone)]
pub struct ProgramConfig {
    // the name of the program, used for naming the thread, logging, etc. Should be unique within a supers application
    name: String,

    // the command used to start the program
    cmd: String,

    // An array of arguments to the program's command.
    args: Vec<String>,

    // The environment variables to set before starting the program, as key-value pairs
    env: HashMap<String, String>,

    // The RestartPolicy for the program
    restartpolicy: RestartPolicy,
}

/// Generate a test application config
pub fn get_test_app_config() -> Vec<ProgramConfig> {
    let p1 = ProgramConfig {
        name: String::from("sleep3"),
        cmd: String::from("/bin/sleep"),
        args: vec![String::from("3")],
        env: HashMap::new(),
        restartpolicy: RestartPolicy::Always,
    };

    let mut envs = HashMap::new();
    envs.insert("NAME".to_string(), "Joe".to_string());

    let p2 = ProgramConfig {
        name: String::from("echo"),
        cmd: String::from("/bin/sh"),
        args: vec![
            String::from("-c"),
            String::from("echo"),
            String::from("Hello, $NAME, from supers"),
        ],
        env: envs,
        restartpolicy: RestartPolicy::Never,
    };

    let mut envs2 = HashMap::new();
    envs2.insert("NAME".to_string(), "Joe".to_string());

    let p3 = ProgramConfig {
        name: String::from("ls"),
        cmd: String::from("ls"),
        args: vec![],
        env: envs2,
        restartpolicy: RestartPolicy::OnError,
    };

    vec![p1, p2, p3]
}

/// Run a program with ProgramConfig, `p`, and supervise it to completion.
pub fn run_program(
    p: ProgramConfig,
    app_state: Arc<Mutex<ApplicationState>>,
    rx: Receiver<i32>,
) -> Result<(), SupersError> {
    loop {
        let mut break_outer_loop = false;
        // Start the program in a child process.
        println!("Starting child process for program {}", p.name);
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

        // inner loop 1: wait for either a command message on the rx channel or for the process to exit.
        // if the process exits, check the exit status and the program config to determine if it should be started again.
        //   if it should be started again, set the break_outer_loop to true; in either case, break out of this loop.
        //   if it shouldn't be started again, we only break out of this first inner loop; the second one will wait for
        //      messages.
        loop {
            let ten_millis = time::Duration::from_millis(10);
            sleep(ten_millis);
            // Check if the process has exited and collect the exit code
            let exit_status = child
                .try_wait()
                .map_err(|e| SupersError::ProgramProcessExitError(p.name.to_string(), e))?;
            // if we got an exit status, then the program has stopped; need to deal with the possibly restart.
            if let Some(exit) = exit_status {
                // the program has exited, so update the status
                let mut a = app_state.lock().unwrap();
                *a.programs
                    .entry(p.name.to_string())
                    .or_insert(ProgramStatus::Stopped) = ProgramStatus::Stopped;
                drop(a);
                // if the program exited with a success, then we only restart if the restart policy is Always
                if exit.success() {
                    println!("Program {} exited successfully.", &p.name);
                    if p.restartpolicy == RestartPolicy::Always {
                        // break the outer loop so that we restart the program
                        break_outer_loop = true;
                        break;
                    }
                    // otherwise, we break out of the inner loop but not the outer loop
                    else {
                        break;
                    }
                }
                // otherwise, the program exited without success, so we restart it if the policy is Always or OnError
                if p.restartpolicy == RestartPolicy::Always
                    || p.restartpolicy == RestartPolicy::OnError
                {
                    // break the outer loop so that we restart the program
                    break_outer_loop = true;
                    break;
                }
                // otherwise, we break out of the inner loop but not the outer loop
                else {
                    break;
                }
            }
            // If we are here, we did not break out of the inner loop 1, so the program is still running.
            // check to see if we have a new message
            let msg = rx.try_recv();
            // ** TODO ** for now, we'll just swallow communication errors.
            if let Ok(m) = msg {
                if m == handlers::START_MSG {
                    // the program is already running, so a START message is a no op; continue to the top of loop1
                    continue;
                } else if m == handlers::STOP_MSG {
                    println!("Program {} received stop message; killing process.", p.name);
                    // for a stop message, we just need to kill the process. We don't want to break from the outer loop
                    // and start the program again but we do want to break from this loop
                    child.kill().expect("Unable to kill child process!");
                    let mut a = app_state.lock().unwrap();
                    *a.programs
                        .entry(p.name.to_string())
                        .or_insert(ProgramStatus::Stopped) = ProgramStatus::Stopped;
                    drop(a);
                    break;
                }
                // otherwise, the message is a RESTART. we need to kill the process and break all the way out of the
                // outer loop to restart the program
                else {
                    println!("Program {} received restart message; killing process and then starting a new one.", p.name);
                    child.kill().expect("Unable to kill child process!");
                    let mut a = app_state.lock().unwrap();
                    *a.programs
                        .entry(p.name.to_string())
                        .or_insert(ProgramStatus::Stopped) = ProgramStatus::Stopped;
                    drop(a);
                    break_outer_loop = true;
                    break;
                }
            }
        }
        // check to see if we are ready to break to the top of the loop and restart the program
        if break_outer_loop {
            continue;
        }

        // inner loop 2:
        // if we are here, the program is stopped. we simply loop, receviing messages until we get a START
        // or RESTART message
        loop {
            // we can just block forever until we get a message
            let msg = rx.recv().unwrap();
            if msg == handlers::STOP_MSG {
                // the program is already stopped, so just loop back up to get the next message
                continue;
            }
            // otherwise, we got a START or a RESTART, so break out of both this loop and the outer loop
            // to restart the program
            else {
                break;
            }
        }
    }
}

/// Starts the threads for all the programs in a specific app config
pub fn start_program_threads(
    app_config: Vec<ProgramConfig>,
    app_state: &Arc<Mutex<ApplicationState>>,
) -> Result<(Vec<JoinHandle<()>>, HashMap<String, Sender<i32>>), SupersError> {
    let mut handles: Vec<JoinHandle<()>> = vec![];
    let mut send_channels = HashMap::new();
    // start a thread for each program in the config
    for program in app_config {
        let p = program.clone();
        let t = thread::Builder::new().name(program.name);
        let program_name = p.name.clone();
        let (tx, rx) = channel::<i32>();
        let app_state_clone = app_state.clone();
        send_channels.insert(program_name.to_string(), tx);
        let handle = t
            .spawn(move || {
                println!("Starting supers thread for program {}...", p.name);
                let _result = run_program(p, app_state_clone, rx);
            })
            .map_err(|e| SupersError::ProgramThreadStartError(program_name, e))?;
        handles.push(handle);
    }

    Ok((handles, send_channels))
}

/// Starts the main server thread, which proides the API for controlling running MereBlocks applications.
pub fn start_server_thread() -> Result<(), SupersError> {
    // todo: need to design how this thread will update the application state/config.
    println!("Starting server thread...");
    Ok(())
}

#[derive(Clone)]
pub struct WebAppState {
    app_state: Arc<Mutex<ApplicationState>>,
    channels: HashMap<String, Sender<i32>>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let app_config = get_test_app_config();
    let app_state = Arc::new(Mutex::new(ApplicationState {
        application_status: ApplicationStatus::Running,
        programs: HashMap::new(),
    }));

    let (_threads, channels) = start_program_threads(app_config, &app_state).unwrap();

    // create the webapp state object
    let webapp_state = WebAppState {
        app_state,
        channels,
    };

    // Start the HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(webapp_state.clone()))
            .service(handlers::ready)
            .service(handlers::get_app_status)
            .service(handlers::get_programs)
            .service(handlers::get_program)
            .service(handlers::start_program)
            .service(handlers::stop_program)
            .service(handlers::restart_program)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await

    // for t in threads {
    //     t.join().unwrap();
    // }
}
