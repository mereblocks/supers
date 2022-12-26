#[cfg(test)]
mod test {
    use anyhow::Result;
    use crossbeam::{
        channel::{unbounded, Sender},
        select,
    };
    use rand::{thread_rng, Rng};
    use std::{
        thread,
        time::{Duration, Instant},
    };

    #[derive(Debug)]
    struct Msg {
        #[allow(dead_code)]
        command: String,
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
        thread::spawn(move || -> Result<()> {
            let mut rng = thread_rng();
            let dur = Duration::from_secs(rng.gen_range(1..10));
            println!(
                "Thread 1 will sleep {} secs and send message 1",
                dur.as_secs()
            );
            thread::sleep(dur);
            s1.send(1)?;
            Ok(())
        });
        thread::spawn(move || -> Result<()> {
            let mut rng = thread_rng();
            let dur = Duration::from_secs(rng.gen_range(1..10));
            println!(
                "Thread 2 will sleep {} secs and send message 2",
                dur.as_secs()
            );
            thread::sleep(dur);
            s2.send(2)?;
            Ok(())
        });
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
        Ok(())
    }
}
