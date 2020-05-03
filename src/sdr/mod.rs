extern crate num;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::FFTplanner;
use std::error::Error;
use std::sync::Arc;

extern crate rtlsdr_mt;
use std::sync::mpsc;

const DEFAULT_N_CHUNKS: u32 = 4;
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

pub struct SdrReader {
    n_chunks: u32,
    n_samples: u32,
    reader: rtlsdr_mt::Reader,
    output_queue: mpsc::Sender<Vec<u8>>,
}

#[derive(Debug)]
pub struct WorkerStats {
    count_in: u64,
    count_out: u64,
}

impl SdrReader {
    pub fn new(reader: rtlsdr_mt::Reader, output_queue: mpsc::Sender<Vec<u8>>) -> SdrReader {
        SdrReader {
            n_chunks: DEFAULT_N_CHUNKS,
            n_samples: DEFAULT_N_SAMPLES,
            reader,
            output_queue,
        }
    }

    pub fn read_samples_loop(&mut self) -> WorkerStats {
        let mut stats = WorkerStats {
            count_in: 0,
            count_out: 0,
        };
        let output_queue = self.output_queue.clone();
        self.reader
            .read_async(self.n_chunks, self.n_samples, |bytes| {
                stats.count_in += 1;
                let v = Vec::from(bytes.clone());
                match transport_samples(v, &output_queue) {
                    Ok(_) => stats.count_out += 1,
                    Err(err) => {
                        eprintln!("Error sending in sdr callback {:?}", err);
                        eprintln!(
                            "Read Sdr samples in = {}, out = {}",
                            stats.count_in, stats.count_out
                        );
                    }
                };
            })
            .unwrap();
        stats
    }
}

fn transport_samples(
    samples: Vec<u8>,
    queue: &mpsc::Sender<Vec<u8>>,
) -> Result<(), std::sync::mpsc::SendError<Vec<u8>>> {
    queue.send(samples)
}

pub struct FFTWorker {
    fft: Arc<dyn rustfft::FFT<f32>>,
    input_queue: mpsc::Receiver<Vec<u8>>,
    output_queue: mpsc::Sender<Vec<(f64, f64)>>,
}

impl FFTWorker {
    pub fn new(
        input_queue: mpsc::Receiver<Vec<u8>>,
        output_queue: mpsc::Sender<Vec<(f64, f64)>>,
    ) -> Self {
        let mut planner = FFTplanner::new(false);
        let fft = planner.plan_fft((DEFAULT_N_SAMPLES / 2) as usize);
        FFTWorker {
            fft,
            input_queue,
            output_queue,
        }
    }

    pub fn compute_fft_loop(&mut self) -> WorkerStats {
        let mut stats = WorkerStats {
            count_in: 0,
            count_out: 0,
        };
        loop {
            let samples = match self.input_queue.try_recv() {
                Ok(samples) => {
                    stats.count_in += 1;
                    samples
                }
                Err(err) => {
                    eprintln!("Error recv in fft_thread {:?}", err);
                    eprintln!(
                        "FFT Worker in_count = {}, out_count = {}",
                        stats.count_in, stats.count_out
                    );
                    return stats;
                }
            };

            let mut iq_samples = complex_from_rtlsdr(&samples);

            let mut output: Vec<Complex<f32>> = vec![Complex::zero(); iq_samples.len()];
            self.fft.process(&mut iq_samples, &mut output);

            let output = output
                .iter()
                .enumerate()
                .map(|(i, c)| (i as f64, to_db(c, iq_samples.len() as f64)))
                .collect();

            match self.output_queue.send(output) {
                Ok(_) => stats.count_out += 1,
                Err(err) => {
                    eprintln!("Error sending in fft_thread {:?}", err);
                    eprintln!(
                        "FFT Worker in_count = {}, out_count = {}",
                        stats.count_in, stats.count_out
                    );
                    return stats;
                }
            };
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_simple() {
        let bytes = vec![0x1, 0x2, 0x3, 0x4];
        let floats = convert_to_floats(&bytes);
        let complex = convert_to_complex(&floats);
        println!("{:?}", complex);
    }
}
