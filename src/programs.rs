use core::time;
use std::{
    collections::HashMap,
    process::{Child, Command, ExitStatus},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use crossbeam::channel::{unbounded, Receiver, Sender};
use tracing::{debug, debug_span, instrument};

use crate::{
    errors::SupersError,
    messages::CommandMsg,
    state::{ApplicationState, ProgramStatus},
    ProgramConfig, RestartPolicy,
};

type SupersChild = Option<Child>;

// Amount of time the command thread will wait for a command message on the command channel.
pub const WAIT_TIMEOUT: time::Duration = time::Duration::from_millis(10);

/// Function to start a program with config given by, `p`, in a child process.
#[instrument(level = "debug")]
pub fn start_child_program(p: &ProgramConfig) -> Result<Child, SupersError> {
    debug!("spawning child");
    Command::new(&p.cmd)
        .args(&p.args)
        .envs(&p.env)
        .spawn()
        .map_err(|e| {
            SupersError::ProgramProcessSpawnError(p.name.to_string(), e)
        })
}

/// Update the status of program with name, `pgm_name`, to status, `status`.
/// This function panics if it cannot lock the app_state object.
#[instrument(level = "debug", skip(app_state))]
pub fn update_pgm_status(
    app_state: Arc<Mutex<ApplicationState>>,
    pgm_name: &str,
    status: ProgramStatus,
) {
    debug!("updating program status");
    let mut a = app_state.lock().unwrap();
    *a.programs.entry(pgm_name.into()).or_insert(status) = status;
}

enum Action {
    ResetChild,
    SpawnChild,
    KillChild,
    ApplyPolicy(ExitStatus),
    UpdateStatus(ProgramStatus),
}

fn run_state_machine_with_effects(
    program_config: &ProgramConfig,
    app_state: Arc<Mutex<ApplicationState>>,
    cmd_tx: Sender<CommandMsg>,
    cmd_rx: Receiver<CommandMsg>,
) -> Result<(), SupersError> {
    let mut current_child: SupersChild = None;
    loop {
        let msg = cmd_rx.recv_timeout(WAIT_TIMEOUT).ok();
        let status =
            get_child_status(&program_config.name, &mut current_child)?;
        let actions = state_machine_step(&status, &msg);
        run_actions(
            &actions,
            &mut current_child,
            &cmd_tx,
            program_config,
            app_state.clone(),
        )?;
    }
}

fn state_machine_step(
    status: &ChildStatus,
    msg: &Option<CommandMsg>,
) -> Vec<Action> {
    match (status, msg) {
        (ChildStatus::NoChild, None) => {
            // There is no child and no command to process.
            // Definitely nothing to do here.
            vec![]
        }
        (ChildStatus::NoChild, Some(CommandMsg::Start)) => {
            vec![
                Action::SpawnChild,
                Action::UpdateStatus(ProgramStatus::Running),
            ]
        }
        (
            ChildStatus::NoChild,
            Some(CommandMsg::Restart | CommandMsg::Stop),
        ) => {
            // If we don't have a child, `Stop` does nothing.
            // TODO: Do we want `Restart` to spawn a child?
            vec![]
        }
        (ChildStatus::Alive, None) => {
            // Everything running smoothly and no command. Don't disturb it :-)
            vec![]
        }
        (ChildStatus::Alive, Some(CommandMsg::Start)) => {
            // Child is running, so no sense in "starting" it. Do nothing.
            vec![]
        }
        (ChildStatus::Alive, Some(CommandMsg::Stop)) => {
            vec![
                Action::KillChild,
                Action::UpdateStatus(ProgramStatus::Stopped),
                Action::ResetChild,
            ]
        }
        (ChildStatus::Alive, Some(CommandMsg::Restart)) => {
            vec![Action::KillChild, Action::SpawnChild]
        }
        (ChildStatus::Exited(code), None) => {
            // The child exited, and there is no command in the queue.
            // Let's apply the policies, if any.
            vec![Action::ApplyPolicy(*code)]
        }
        (ChildStatus::Exited(_), Some(CommandMsg::Stop)) => {
            // Child has exited, so we ignore the `Stop` command
            vec![]
        }
        (
            ChildStatus::Exited(_),
            Some(CommandMsg::Start | CommandMsg::Restart),
        ) => {
            // Child has exited, so we ignore the `Stop` command
            vec![
                Action::SpawnChild,
                Action::UpdateStatus(ProgramStatus::Running),
            ]
        }
    }
}

fn run_actions(
    actions: &[Action],
    child: &mut SupersChild,
    tx: &Sender<CommandMsg>,
    program_config: &ProgramConfig,
    app_state: Arc<Mutex<ApplicationState>>,
) -> Result<(), SupersError> {
    for action in actions {
        run_action(action, child, tx, program_config, app_state.clone())?;
    }
    Ok(())
}

fn run_action(
    action: &Action,
    child: &mut SupersChild,
    tx: &Sender<CommandMsg>,
    program_config: &ProgramConfig,
    app_state: Arc<Mutex<ApplicationState>>,
) -> Result<(), SupersError> {
    match action {
        Action::ResetChild => {
            *child = None;
        }
        Action::SpawnChild => {
            *child = Some(start_child_program(program_config)?);
        }
        Action::KillChild => {
            child
                .as_mut()
                .map(|c| {
                    c.kill().map_err(|e| {
                        SupersError::ProgramProcessKillError(
                            program_config.name.clone(),
                            e,
                        )
                    })
                })
                .unwrap_or_else(|| {
                    unreachable!(
                        "Asked to kill non-existent child. This is a bug."
                    )
                })?;
        }
        Action::ApplyPolicy(code) => {
            match program_config.restartpolicy {
                RestartPolicy::Always => {
                    // Under this policy, we **always** restart
                    tx.send(CommandMsg::Start)?;
                }
                RestartPolicy::Never => {
                    // Do nothing, keep in `Exited` state.
                }
                RestartPolicy::OnError => {
                    // We restart if `code` is an error
                    if !code.success() {
                        debug!("program exited with error. Restarting");
                        tx.send(CommandMsg::Start)?;
                    }
                }
            }
        }
        Action::UpdateStatus(status) => {
            update_pgm_status(app_state, &program_config.name, *status);
        }
    }
    Ok(())
}

// Process next step in the state machine.
// The states of the machine are values of type `ChildStatus`.
// The transitions are generated by values of type `Option<CommandMsg>` plus
// automatic transitions defined by the policies (for example: from `Exited(.)`
// to `Alive` when the policy is RestartAlways).
//
// We pass a sender for `CommandMsg` so we can queue new commands. For example,
// a RESTART can be processed by sending two messages in sequence to `cmd_tx`: STOP,
// and then START.
#[instrument(level = "debug", skip_all, fields(program = p.name, mesg = ?msg))]
fn run_state_machine(
    child: SupersChild,
    msg: Option<CommandMsg>,
    cmd_tx: Sender<CommandMsg>,
    p: &ProgramConfig,
    app_state: Arc<Mutex<ApplicationState>>,
) -> Result<SupersChild, SupersError> {
    let mut child = child;
    let status = get_child_status(&p.name, &mut child)?;
    let _span = debug_span!("step", ?status, ?msg).entered();
    debug!("state machine step");
    Ok(match (status, msg) {
        (ChildStatus::NoChild, None) => {
            // There is no child and no command to process.
            // Definitely nothing to do here.
            child
        }
        (ChildStatus::NoChild, Some(CommandMsg::Start)) => {
            // This is the only place where we actually spawn a child
            update_pgm_status(app_state, &p.name, ProgramStatus::Running);
            Some(start_child_program(p)?)
        }
        (
            ChildStatus::NoChild,
            Some(CommandMsg::Stop | CommandMsg::Restart),
        ) => {
            // If we don't have a child, `Stop` and `Restart` do nothing
            child
        }
        (ChildStatus::Alive, None) => {
            // Everything running smoothly and no command. Don't disturb it :-)
            child
        }
        (ChildStatus::Alive, Some(CommandMsg::Start)) => {
            // Child is running, so no sense in "starting" it. Do nothing.
            child
        }
        (ChildStatus::Alive, Some(CommandMsg::Stop)) => {
            // We stop the child. This is the only place where we kill the child.
            if let Some(c) = child.as_mut() {
                debug!("stopping child");
                c.kill().map_err(|e| {
                    SupersError::ProgramProcessKillError(p.name.clone(), e)
                })?;
                update_pgm_status(app_state, &p.name, ProgramStatus::Stopped);
            } else {
                unreachable!("If `get_child_status` returned `Alive`, then `child` is not `None`");
            }
            // The new child is `None`
            None
        }
        (ChildStatus::Alive, Some(CommandMsg::Restart)) => {
            // For restarting, we schedule two messages: Stop & Start
            debug!("child is alive, sending Stop & Start");
            cmd_tx.send(CommandMsg::Stop)?;
            cmd_tx.send(CommandMsg::Start)?;
            // The new child is still the same. The next iterations will change
            // it when they process the Stop and the Start.
            child
        }
        (ChildStatus::Exited(code), None) => {
            // The child exited, and there is no command in the queue.
            // Let's apply the policies, if any.
            debug!(?code, "program exited");
            update_pgm_status(app_state, &p.name, ProgramStatus::Stopped);
            match p.restartpolicy {
                RestartPolicy::Always => {
                    // Under this policy, we **always** restart
                    debug!("restart policy is Always. Restarting");
                    cmd_tx.send(CommandMsg::Start)?;
                }
                RestartPolicy::Never => {
                    debug!("restart policy is Never. Doing nothing");
                    // Do nothing, keep in `Exited` state.
                }
                RestartPolicy::OnError => {
                    // We restart if `code` is an error
                    if !code.success() {
                        debug!("program exited with error. Restarting");
                        cmd_tx.send(CommandMsg::Start)?;
                    }
                }
            }
            // Keep the same child. It will be updated after processing the
            // scheduled messages.
            child
        }
        (ChildStatus::Exited(_), Some(CommandMsg::Stop)) => {
            // Child has exited, so we ignore the `Stop` command
            child
        }
        (
            ChildStatus::Exited(_),
            Some(CommandMsg::Start | CommandMsg::Restart),
        ) => {
            // We got a command to start or restart an exited child.
            // We resend the `Start` message and reset the child.
            debug!("resetting child and sending Start command");
            cmd_tx.send(CommandMsg::Start)?;
            None
        }
    })
}

#[derive(Debug)]
enum ChildStatus {
    NoChild,
    Alive,
    Exited(ExitStatus),
}

// Get status of child with the meaning:
//   NoChild   -> we still don't have any child spawned
//   Alive     -> the child is running
//   Exited(s) -> the child exited with status `s`
// Return `Err` if we got an error while retrieving status.
// This function **does not** block.
fn get_child_status(
    name: &str,
    child: &mut SupersChild,
) -> Result<ChildStatus, SupersError> {
    child
        .as_mut()
        .map(|child| {
            child.try_wait().map(|status| {
                status
                    .map(ChildStatus::Exited)
                    .unwrap_or(ChildStatus::Alive)
            })
        })
        .unwrap_or_else(|| Ok(ChildStatus::NoChild))
        .map_err(|e| {
            SupersError::ProgramCheckProcessStatusError(name.into(), e)
        })
}

/// Function to start and monitor a process while also monitoring and processing the
/// associated command channel for a specific program.
///
#[instrument(level = "debug", skip_all, fields(program = program_config.name))]
pub fn pgm_thread(
    program_config: &ProgramConfig,
    app_state: Arc<Mutex<ApplicationState>>,
    cmd_tx: Sender<CommandMsg>,
    cmd_rx: Receiver<CommandMsg>,
) -> Result<(), SupersError> {
    debug!("starting program thread");
    let mut current_child: SupersChild = None;
    loop {
        let msg = cmd_rx.recv_timeout(WAIT_TIMEOUT).ok();
        let _span = debug_span!("message_span", ?msg).entered();
        debug!("received command message");
        // Run next step of state machine
        // and update `current_child` if the state changed
        current_child = run_state_machine(
            current_child,
            msg,
            cmd_tx.clone(),
            program_config,
            app_state.clone(),
        )?;
    }
}

/// Type alias for the start_program_threads return type; A tuple type containing the thread handles for each thread
/// started as well as a hashmap of the command channels created for each program in the App config.
type ProgramControls = (
    Vec<JoinHandle<Result<(), SupersError>>>,
    HashMap<String, Sender<CommandMsg>>,
);

/// Main entrypoint for the programs.rs module; For each program in the app_config, this function:
/// 1) creates a command channel to process commands from the administrative API
/// 2) starts a thread to run and monitor the program, passing in the command channel.
#[instrument(level = "debug", skip_all)]
pub fn start_program_threads(
    app_config: Vec<ProgramConfig>,
    app_state: &Arc<Mutex<ApplicationState>>,
) -> Result<ProgramControls, SupersError> {
    let mut handles = vec![];
    let mut send_channels = HashMap::new();
    // start a thread for each program in the config
    debug!("starting threads for all programs");
    for program in app_config {
        debug!(program = program.name, "starting thread for program");
        let (tx, rx) = unbounded::<CommandMsg>();
        {
            let program = program.clone();
            let program_name = program.name.clone();
            let tx = tx.clone();
            let app_state = app_state.clone();
            let handle = thread::Builder::new()
                .name(program_name.clone())
                .spawn(move || -> Result<(), SupersError> {
                    pgm_thread(&program, app_state, tx, rx)
                })
                .map_err(|e| {
                    SupersError::ProgramThreadStartError(program_name, e)
                })?;
            handles.push(handle);
        }
        send_channels.insert(program.name.clone(), tx);
    }

    Ok((handles, send_channels))
}

#[cfg(test)]
mod test {
    use crate::{
        get_test_app_config, log::init_tracing, messages::CommandMsg,
        state::ApplicationState,
    };
    use anyhow::Result;
    use crossbeam::channel::{select, unbounded};
    use std::{
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };
    use tracing::info;

    use super::pgm_thread;
    use test_log::test;

    #[test]
    fn test_foo() -> Result<()> {
        info!("in test foo");
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_state_machine() -> Result<()> {
        init_tracing();
        let p = get_test_app_config().programs[2].clone();
        let app_state = Arc::new(Mutex::new(ApplicationState::default()));
        let (s, r) = unbounded();
        let t;
        {
            let s = s.clone();
            let app_state = app_state.clone();
            t = thread::spawn(move || -> Result<()> {
                Ok(pgm_thread(&p, app_state, s, r)?)
            });
        }
        s.send(CommandMsg::Start)?;
        thread::sleep(Duration::from_secs(2));
        println!("State: {:?}", app_state.lock().unwrap());
        s.send(CommandMsg::Start)?;
        thread::sleep(Duration::from_secs(2));
        println!("State: {:?}", app_state.lock().unwrap());
        t.join().unwrap().unwrap();
        Ok(())
    }

    #[test]
    #[ignore]
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
