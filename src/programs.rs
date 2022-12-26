use std::thread;

use crossbeam::{channel::{unbounded, select, Sender, Receiver}};


pub fn test_channels() -> () {
    let (s_pgm, r_pgm) = unbounded::<i32>();
    let (s_cmd, r_cmd) = unbounded::<i32>();
    let (s_threads, r_threads) = unbounded::<i32>();

    let START = 11;
    let STOP = 12;
    let RESTART = 13;

    let pgms_thread = thread::spawn( move || {
        // start program, get a child, send it over the programs channel ---
        let mut child = 1;
        loop {
            if child > 3 {
                break;
            }
            let _r = s_pgm.send(child);
            // would wait for child to exit..
            let msg = r_threads.recv().unwrap();
            println!("pgrms_thread got a message on the threads channel: {:?}", msg);

            child += 1;

        }


    });

    let cmds_thread = thread::spawn( move || {
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