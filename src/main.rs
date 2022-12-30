use actix_web::web::Data;
use actix_web::{App, HttpServer};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crossbeam::channel::Sender;

use errors::SupersError;
use messages::CommandMsg;
use state::{ApplicationState, ApplicationStatus};

use programs::start_program_threads;

mod errors;
mod handlers;
mod programs;
mod state;
// TODO: This is just a module for playing with ideas. Remove before production.
mod messages;
mod playground;

/// These are the available restart policies for programs
#[derive(Clone, PartialEq, Debug)]
pub enum RestartPolicy {
    // Always restart the program after it exits, regardless of exit status
    Always,

    // Never restart the program, regardless of exist status
    Never,

    // Restart the program if it exited with a non-success status, otherwise, do not restart
    OnError,
}

/// Configuration for a program to be launched and supervised by supers.
#[derive(Clone, Debug)]
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

/// Starts the main server thread, which proides the API for controlling running MereBlocks applications.
pub fn start_server_thread() -> Result<(), SupersError> {
    // todo: need to design how this thread will update the application state/config.
    println!("Starting server thread...");
    Ok(())
}

#[derive(Clone)]
pub struct WebAppState {
    app_state: Arc<Mutex<ApplicationState>>,
    channels: HashMap<String, Sender<CommandMsg>>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // get the config for this Mereblocks application and create the app_state container
    let app_config = get_test_app_config();
    let app_state = Arc::new(Mutex::new(ApplicationState {
        application_status: ApplicationStatus::Running,
        programs: HashMap::new(),
    }));

    // start the threads for the programs configured the application
    let (_threads, channels) = start_program_threads(app_config, &app_state).unwrap();

    // send a start message to all programs
    for sx in channels.values() {
        let _r = sx.send(CommandMsg::Start);
    }
    // create the webapp state object with the command hannels used to communicate with the threads
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
