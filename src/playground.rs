#[cfg(test)]
mod test {
    use actix_web::guard::Options;
    use anyhow::Result;
    use crossbeam::{
        channel::{unbounded, Select, Sender},
        select,
    };
    use rand::{thread_rng, Rng};
    use std::{
        collections::HashSet,
        thread,
        time::{Duration, Instant},
    };

    #[derive(Debug)]
    struct Msg {
        #[allow(dead_code)]
        command: String,
    }

    fn test_child_match() -> Result<()> {
        let mut c: Option<std::process::Child> = None;
        let child = std::process::Command::new("ls").spawn().unwrap();
        let mut rng = thread_rng();
        let chance = rng.gen_range(1..2);
        if chance < 2 {
            c = Some(child);
        }
        loop {
            match &c {
                Some(ch) => {
                    let status = ch.wait();
                    break;
                }
                None => {
                    println!("No child");
                }
            };
        }

        Ok(())
    }

    #[test]
    fn test_basic_channels() -> Result<()> {
        // Test send/receive via channel with integers
        let (s, r) = unbounded();
        s.send(4)?;
        let x = r.recv()?;
        dbg!(x);

        // Test send/receive via channel with a struct
        let (s, r) = unbounded();
        s.send(Msg {
            command: "foo".into(),
        })?;
        let x = r.recv()?;
        dbg!(x);
        Ok(())
    }

    #[test]
    fn test_thread_with_channel() -> Result<()> {
        // Test sending from a thread and receiving in main thread
        // Note that can give an explicit signature to the thread to have
        // a `Result<...>` so we can use the `?` operator inside the thread.
        // We capture the `Result<...>` with the `.join()` in the main thread.
        let (s, r) = unbounded();
        let t = thread::spawn(move || -> Result<()> {
            s.send(4)?;
            println!("Thread sent 4");
            Ok(())
        });
        let x = r.recv()?;
        println!("Main thread got {x}");
        t.join().expect("thread did panic")?;
        Ok(())
    }

    fn random_sleeper(index: i32, respond_to: Sender<i32>) -> Result<()> {
        let mut rng = thread_rng();
        let dur = Duration::from_secs(rng.gen_range(1..10));
        println!(
            "Thread {index} will sleep {} secs and send message {index}",
            dur.as_secs()
        );
        thread::sleep(dur);
        respond_to.send(index)?;
        Ok(())
    }

    #[test]
    fn test_select() -> Result<()> {
        // Test a very basic `select`. Two threads sleep and the first one
        // to wake up sends a message and everything exits.
        // Display elapsed time to ensure we got the message at the right time.
        let (s1, r1) = unbounded();
        let (s2, r2) = unbounded();
        let start = Instant::now();
        thread::spawn(move || random_sleeper(1, s1));
        thread::spawn(move || random_sleeper(2, s2));
        select! {
            recv(r1) -> msg => println!("From thread 1 got: {:?}", msg),
            recv(r2) -> msg => println!("From thread 2 got: {:?}", msg),
            default(Duration::from_secs(7)) => println!("7 seconds passed")
        }
        println!("Ellapsed time: {}", start.elapsed().as_secs());
        Ok(())
    }

    #[test]
    fn test_dynamic_select() -> Result<()> {
        // Test using `Select` with a dynamic list of operations.
        let mut sel = Select::new();
        let (s1, r1) = unbounded();
        let (s2, r2) = unbounded();
        let index1 = sel.recv(&r1);
        let index2 = sel.recv(&r2);
        println!("Added operation for thread 1 with index: {index1}");
        println!("Added operation for thread 2 with index: {index2}");
        let mut indices = HashSet::from([index1, index2]);

        let start = Instant::now();
        thread::spawn(move || random_sleeper(1, s1));
        thread::spawn(move || random_sleeper(2, s2));
        loop {
            let oper = sel.select();
            println!("Selected operation: {}", oper.index());
            if oper.index() == index1 {
                match oper.recv(&r1) {
                    Ok(x) => println!("From thread 1 got {x}"),
                    Err(_) => {
                        println!("Thread 1 got error, removing it");
                        println!("Time elapsed from start: {}", start.elapsed().as_secs());
                        sel.remove(index1);
                        indices.remove(&index1);
                    }
                };
            } else if oper.index() == index2 {
                match oper.recv(&r2) {
                    Ok(x) => println!("From thread 1 got {x}"),
                    Err(_) => {
                        println!("Thread 2 got error, removing it");
                        println!("Time elapsed from start: {}", start.elapsed().as_secs());
                        sel.remove(index2);
                        indices.remove(&index2);
                    }
                };
            }
            if indices.is_empty() {
                break;
            }
        }
        Ok(())
    }
}
