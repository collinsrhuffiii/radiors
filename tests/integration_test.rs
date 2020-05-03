#[cfg(test)]
extern crate radiors;

use radiors::sdr;
use std::sync::mpsc::channel;
use std::thread;

#[test]
fn keep_up() {
    let (mut controller, reader) = rtlsdr_mt::open(0).unwrap();
    let (samples_tx, samples_rx) = channel();
    let (fft_tx, fft_rx) = channel();

    let mut sdr_reader = sdr::SdrReader::new(reader, samples_tx);
    let mut fft_worker = sdr::FFTWorker::new(samples_rx, fft_tx);

    let h1 = thread::Builder::new()
        .name("sdr reader".to_string())
        .spawn(move || {
            sdr_reader.read_samples_loop();
        })
        .unwrap();

    let h2 = thread::Builder::new()
        .name("fft worker".to_string())
        .spawn(move || fft_worker.compute_fft_loop())
        .unwrap();

    controller.cancel_async_read();
    drop(controller);
    let reader_stats = h1.join().unwrap();
    let fft_stats = h2.join().unwrap();

    println!("{:?}", reader_stats);
    println!("{:?}", fft_stats);

    assert!(false);
}
