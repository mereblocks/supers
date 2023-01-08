use crate::config::{ApplicationConfig, ProgramConfig, RestartPolicy};
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use log::init_tracing;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::info;
use tracing_actix_web::TracingLogger;

use crossbeam::channel::Sender;

use errors::SupersError;
use messages::CommandMsg;
use state::{ApplicationState, ApplicationStatus};

use programs::start_program_threads;

mod config;
mod errors;
mod handlers;
mod log;
mod messages;
mod programs;
mod state;
// TODO: This is just a module for playing with ideas. Remove before production.
mod playground;

/// Generate a test application config
pub fn get_test_app_config() -> ApplicationConfig {
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

    ApplicationConfig {
        app_name: "Test App".to_string(),
        programs: vec![p1, p2, p3],
        ..Default::default()
    }
}

/// Starts the main server thread, which proides the API for controlling running MereBlocks applications.
pub fn start_server_thread() -> Result<(), SupersError> {
    // todo: need to design how this thread will update the application state/config.
    info!("Starting server thread...");
    Ok(())
}

#[derive(Clone)]
pub struct WebAppState {
    app_state: Arc<Mutex<ApplicationState>>,
    channels: HashMap<String, Sender<CommandMsg>>,
}

#[actix_web::main]
async fn main() -> Result<(), SupersError> {
    init_tracing();

    let app_config = ApplicationConfig::from_sources()?;

    // create the app_state container with statuses for the application status and the programs
    let app_state = Arc::new(Mutex::new(ApplicationState {
        application_status: ApplicationStatus::Running,
        programs: HashMap::new(),
    }));

    // start the threads for the programs configured the application
    let (_threads, channels) =
        start_program_threads(app_config.programs, &app_state).unwrap();

    // send a start message to all programs
    for sx in channels.values() {
        sx.send(CommandMsg::Start)?;
    }
    // create the webapp state object with the command hannels used to communicate with the threads
    let webapp_state = WebAppState {
        app_state,
        channels,
    };

    // Start the HTTP server
    HttpServer::new(move || {
        App::new()
            .wrap(actix_web::middleware::Logger::default())
            .wrap(TracingLogger::default())
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
    .await?;

    Ok(())
    // for t in threads {
    //     t.join().unwrap();
    // }
}
