use std::{thread, time};
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc::{channel, Receiver}};
use std::io::prelude::*;
use std::fs::File;


const MAIN_SLEEP_TIME: u64 = 2500; //Number of micros to sleep for

static DONE: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
struct DataContainer{
    internal_count: u64,
    fake_vector: [u8; 10240]
}

fn main() {
    ctrlc::set_handler(|| DONE.store(true, Ordering::SeqCst));

    let mut loop_counter: u64 = 0;
    let mut fake_counter: u8 = 0;
    let (datasender, datareceiver) = channel::<DataContainer>();
    //The bug causes this unbounded channel to grow infinitely and cause an OOM error as t => inf
    let mut data = DataContainer {
        internal_count: 0,
        fake_vector: [127; 10240]
    };

    let write_thread = thread::spawn(|| {
        write_thread(datareceiver);
    });

    while !DONE.load(Ordering::Relaxed) {

        let loop_start = time::Instant::now();
        
        data = DataContainer{
            internal_count: loop_counter,
            fake_vector: [fake_counter; 10240]
        };
        loop_counter += 1;
        fake_counter = fake_counter.wrapping_add(1);

        datasender.send(data);

        thread::sleep(time::Duration::from_micros(MAIN_SLEEP_TIME))
    }

}

fn write_thread(receiver: Receiver<DataContainer>) {
    let mut write_counter = 0;
    let mut file = File::create("output.bin").unwrap();
    while !DONE.load(Ordering::Relaxed) {
        let now = time::Instant::now();
        let reecived_data = receiver
            .recv_timeout(time::Duration::from_millis(50)).unwrap();
        file.write_all(&reecived_data.fake_vector);
        println!("Wrote loop {}! This took {} us!", write_counter, now.elapsed().as_micros());
        //This println will print something akin to "Wrote loop [n]! This took [~MAIN_SLEEP_TIME] us!"
        write_counter += 1;
    }
}
