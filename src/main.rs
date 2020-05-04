mod sdr;
mod ui;
use std::thread;
extern crate spsc_bip_buffer;

use spsc_bip_buffer::bip_buffer_with_len;

fn main() {
    let (sdr_controller, mut sdr_reader) = rtlsdr_mt::open(0).unwrap();
    let buf_len = sdr::DEFAULT_N_SAMPLES * 2;
    let (mut samp_buf_writer, samp_buf_reader) = bip_buffer_with_len(buf_len as usize);

    let fft_worker = sdr::FFTWorker::new(samp_buf_reader);

    let mut fft_app = ui::FFTApp::new(fft_worker, sdr_controller);

    let h1 = thread::Builder::new()
        .name("sdr reader".to_string())
        .spawn(move || {
            sdr::read_samples(
                &mut sdr_reader,
                &mut samp_buf_writer,
                sdr::DEFAULT_N_BUFFERS,
                sdr::DEFAULT_N_SAMPLES,
            );
        })
        .unwrap();

    ui::start_ui(&mut fft_app).unwrap();
    h1.join().unwrap();
}
