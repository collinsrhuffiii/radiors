mod sdr;
mod ui;
use std::error::Error;
use std::sync::mpsc::channel;
use std::thread;

fn main() {
    let (controller, reader) = rtlsdr_mt::open(0).unwrap();
    let (samples_tx, samples_rx) = channel();
    let (fft_tx, fft_rx) = channel();

    let mut sdr_reader = sdr::SdrReader::new(reader, samples_tx);

    let mut fft_worker = sdr::FFTWorker::new(samples_rx, fft_tx);

    let mut fft_app = ui::FFTApp::new(fft_rx, controller);

    let h1 = thread::Builder::new()
        .name("sdr reader".to_string())
        .spawn(move || {
            sdr_reader.read_samples_loop();
        })
        .unwrap();

    let h3 = thread::Builder::new()
        .name("fft worker".to_string())
        .spawn(move || fft_worker.compute_fft_loop())
        .unwrap();

    ui::start_ui(&mut fft_app).unwrap();
    h3.join().unwrap();
}
