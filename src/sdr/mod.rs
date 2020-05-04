extern crate num;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::FFTplanner;
use std::mem;
use std::sync::Arc;

extern crate rtlsdr_mt;
use spsc_bip_buffer::{BipBufferReader, BipBufferWriter};

pub const DEFAULT_N_BUFFERS: u32 = 16;
pub const DEFAULT_N_SAMPLES: u32 = 32768;
pub const DEFAULT_CENTER_FREQUENCY: u32 = 100_259_009;
pub const DEFAULT_BANDWIDTH: u32 = 200_000;
pub const DEFAULT_SAMPLE_RATE: u32 = 2_560_000;
pub const RTLSDR_MAX_BANDWIDTH: u32 = 2_400_000;
use rtlsdr_mt::Controller;

pub fn set_controller_defaults(controller: &mut Controller) {
    controller.enable_agc().unwrap();
    controller.set_ppm(-2).unwrap();
    controller
        .set_center_freq(DEFAULT_CENTER_FREQUENCY - DEFAULT_BANDWIDTH)
        .unwrap();
    controller.set_bandwidth(DEFAULT_BANDWIDTH).unwrap();
    controller.set_sample_rate(DEFAULT_SAMPLE_RATE).unwrap();
}

type IQSamples = Vec<Complex<f32>>;

pub fn read_samples(
    sdr_reader: &mut rtlsdr_mt::Reader,
    buf_writer: &mut BipBufferWriter,
    n_buffers: u32,
    buf_size: u32,
) -> usize {
    let mut count = 0;
    let _ = sdr_reader.read_async(n_buffers, buf_size, |buf| {
        match buf_writer.reserve(buf.len()) {
            Some(mut reservation) => {
                count += buf.len();
                eprintln!("read count = {}", count);
                reservation.copy_from_slice(buf);
                reservation.send();
            }
            None => {
                eprintln!("reader no room");
            }
        }
    });
    mem::forget(sdr_reader);
    count
}

pub struct FFTWorker {
    fft: Arc<dyn rustfft::FFT<f32>>,
    input_queue: BipBufferReader,
    count: usize,
}

impl FFTWorker {
    pub fn new(input_queue: BipBufferReader) -> Self {
        let mut planner = FFTplanner::new(false);
        let fft = planner.plan_fft((DEFAULT_N_SAMPLES / 2) as usize);
        FFTWorker {
            fft,
            input_queue,
            count: 0,
        }
    }

    pub fn compute_fft(&mut self) -> Vec<(f64, f64)> {
        while self.input_queue.valid().len() < DEFAULT_N_SAMPLES as usize {}
        let samples = &self.input_queue.valid()[..DEFAULT_N_SAMPLES as usize];

        let mut iq_samples = complex_from_rtlsdr(&samples);

        let mut output: Vec<Complex<f32>> = vec![Complex::zero(); iq_samples.len()];
        self.fft.process(&mut iq_samples, &mut output);

        let output: Vec<(f64, f64)> = output
            .into_iter()
            .enumerate()
            .map(|(i, c)| (i as f64, to_db(&c, iq_samples.len() as f64)))
            .collect();

        self.count += output.len();
        eprintln!("fft count  = {}", 2 * self.count);
        self.input_queue.consume(DEFAULT_N_SAMPLES as usize);

        output
    }
}

fn to_db(c: &Complex<f32>, n_samples: f64) -> f64 {
    20.0 * (c.norm() as f64 / n_samples).log(10.0)
}

fn convert_to_floats(bytes: &[u8]) -> Vec<f32> {
    let mut floats: Vec<f32> = Vec::new();
    for b in bytes.iter() {
        floats.push(*b as f32);
    }
    floats
}

fn convert_to_complex(iq_samples: &[f32]) -> Vec<Complex<f32>> {
    let mut iq: Vec<Complex<f32>> = Vec::new();

    for i in (0..iq_samples.len() - 1).step_by(2) {
        let mut c = Complex::new(iq_samples[i], iq_samples[i + 1]);
        c = c / 127.5;
        c = c - Complex::new(1.0, 1.0);
        iq.push(c);
    }
    iq
}

fn complex_from_rtlsdr(buf: &[u8]) -> IQSamples {
    let f = convert_to_floats(buf);
    convert_to_complex(&f)
}
