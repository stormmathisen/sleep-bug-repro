use std::fs::File;
use std::io::prelude::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{sync_channel, Receiver, TrySendError},
};
use std::time::Duration;
use std::{thread, time};

use anyhow::{Context, Result};

const MAIN_SLEEP_TIME: Duration = Duration::from_micros(2500);

static DONE: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
struct DataContainer {
    internal_count: u64,
    fake_vector: [u8; 10240],
}

fn main() -> Result<()> {
    ctrlc::set_handler(|| DONE.store(true, Ordering::SeqCst))?;

    let mut loop_counter: u64 = 0;
    let mut fake_counter: u8 = 0;
    let (datasender, datareceiver) = sync_channel::<DataContainer>(4);

    let file = File::create("output.bin").context("Couldn't create output file")?;

    let write_thread = thread::spawn(move || write_thread(datareceiver, file));

    while !DONE.load(Ordering::Relaxed) {
        let end_at = time::Instant::now() + MAIN_SLEEP_TIME;

        let data = DataContainer {
            internal_count: loop_counter,
            fake_vector: [fake_counter; 10240],
        };
        loop_counter += 1;
        fake_counter = fake_counter.wrapping_add(1);

        match datasender.try_send(data) {
            Ok(()) => {} // cool
            Err(TrySendError::Full(_)) => {
                println!("DANGER WILL ROBINSON - writer not keeping up!")
            }
            Err(TrySendError::Disconnected(_)) => {
                // The receiving side hung up!
                // Bounce out of the loop to see what error it had.
                break;
            }
        }

        while time::Instant::now() < end_at {
            std::hint::spin_loop();
        }
    }

    drop(datasender);
    write_thread.join().expect("Couldn't join writer")?;

    Ok(())
}

fn write_thread(receiver: Receiver<DataContainer>, mut file: File) -> Result<()> {
    // Average the last 1000 waits
    let mut waits: [u16; 1000] = [0; 1000];
    let mut waits_wrapped = false;
    let mut wait_index: usize = 0;

    let mut start = time::Instant::now();

    while let Ok(received_data) = receiver.recv() {
        let wait_done = time::Instant::now();
        let wait = (wait_done - start).as_micros();

        assert!(wait < u16::MAX as u128);
        waits[wait_index] = wait as u16;
        wait_index += 1;
        if wait_index >= waits.len() {
            wait_index = 0;
            waits_wrapped = true;
        }

        let waits_to_average = if waits_wrapped {
            &waits
        } else {
            &waits[0..wait_index]
        };

        let avg = waits_to_average.iter().fold(0u64, |acc, w| acc + *w as u64)
            / waits_to_average.len() as u64;

        file.write_all(&received_data.fake_vector)
            .context("Couldn't write output")?;
        let wrote = (time::Instant::now() - wait_done).as_micros();

        println!(
            "Wrote {}! (waited {} us ({} avg), wrote {} us)",
            received_data.internal_count, wait, avg, wrote
        );

        // Account for the time spent blocking for input
        start = time::Instant::now();
    }
    Ok(())
}
