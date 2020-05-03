use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::Backend,
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Axis, Block, Borders, Chart, Dataset, Marker, Paragraph, Text},
    Frame, Terminal,
};

mod util;
use std::error::Error;
use std::io;
use std::sync::mpsc;
use util::event::Event;

use super::sdr::set_controller_defaults;
use super::sdr::RTLSDR_MAX_BANDWIDTH;
use rtlsdr_mt::Controller;

pub struct FFTApp {
    pub controller: Controller,
    fft_output_queue: mpsc::Receiver<Vec<(f64, f64)>>,
    data: Vec<(f64, f64)>,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    in_count: u64,
}

impl FFTApp {
    pub fn new(fft_output_queue: mpsc::Receiver<Vec<(f64, f64)>>, controller: Controller) -> Self {
        let mut app = FFTApp {
            controller,
            fft_output_queue,
            data: vec![],
            x_min: 0.0,
            x_max: 0.0,
            y_min: 0.0,
            y_max: 0.0,
            in_count: 0,
        };

        set_controller_defaults(&mut app.controller);
        app
    }

    fn update(&mut self) {
        while let Ok(data) = self.fft_output_queue.try_recv() {
            self.in_count += 1;
            self.data = data;
        }
        self.y_min = self.data.iter().map(|(_, c)| *c).fold(0. / 0., f64::min);
        self.y_max = self.data.iter().map(|(_, c)| *c).fold(0. / 0., f64::max);
        self.x_min = self.controller.center_freq() as f64;
        self.x_max = self.controller.center_freq() as f64 + RTLSDR_MAX_BANDWIDTH as f64;
    }
}

fn draw_fft_chart<B>(f: &mut Frame<B>, pos: Rect, app: &FFTApp)
where
    B: Backend,
{
    let y_min = -75.0;
    let y_max = -25.0;

    let x_labels = [
        format!("{0:.2}Hz", app.x_min),
        format!("{0:.2}Hz", app.x_max),
    ];

    let y_labels = [format!("{0:.2}dB", y_min), format!("{0:.2}dB", y_max)];

    let dataset = [Dataset::default()
        .name("FFT")
        .marker(Marker::Dot)
        .style(Style::default().fg(Color::Cyan))
        .data(&app.data)];

    let fft_chart = Chart::default()
        .block(
            Block::default()
                .title("FFT")
                .title_style(Style::default().fg(Color::Cyan).modifier(Modifier::BOLD))
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title("X Axis")
                .style(Style::default().fg(Color::Gray))
                .labels_style(Style::default().modifier(Modifier::ITALIC))
                .bounds([0.0, app.data.len() as f64])
                .labels(&x_labels),
        )
        .y_axis(
            Axis::default()
                .title("Y Axis")
                .style(Style::default().fg(Color::Gray))
                .labels_style(Style::default().modifier(Modifier::ITALIC))
                .bounds([y_min, y_max])
                .labels(&y_labels),
        )
        .datasets(&dataset);

    f.render_widget(fft_chart, pos);
}

enum InputMode {
    Normal,
    CenterFreq,
    SampleRate,
    Bandwidth,
}

/// App holds the state of the application
struct App {
    /// Current value of the input box
    input: String,
    /// Current input mode
    input_mode: InputMode,
}

impl Default for App {
    fn default() -> App {
        App {
            input: String::new(),
            input_mode: InputMode::Normal,
        }
    }
}

fn draw_input<B>(f: &mut Frame<B>, help_pos: Rect, input_pos: Rect, app: &App)
where
    B: Backend,
{
    let msg = match app.input_mode {
        InputMode::Normal => "Press q to exit, f to enter center frequency, b to enter bandwidth, s to enter sample rate",
        InputMode::CenterFreq => "Press Esc to cancel, Enter to set the center frequency",
        InputMode::Bandwidth => "Press Esc to cancel, Enter to set the bandwidth",
        InputMode::SampleRate => "Press Esc to cancel, Enter to set the sample rate",
    };

    let text = [Text::raw(msg)];
    let help_message = Paragraph::new(text.iter());
    f.render_widget(help_message, help_pos);

    let text = [Text::raw(&app.input)];
    let input = Paragraph::new(text.iter())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, input_pos);
}

pub fn start_ui(fft_app: &mut FFTApp) -> Result<(), Box<dyn Error>> {
    let stdout = io::stdout().into_raw_mode().unwrap();
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.hide_cursor().unwrap();

    let mut events = util::event::Events::new();

    let mut app = App::default();

    loop {
        terminal.draw(|mut f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Length(1),   // help msg
                        Constraint::Length(3),   //input
                        Constraint::Ratio(1, 1), //fft plot
                    ]
                    .as_ref(),
                )
                .split(size);

            draw_input(&mut f, chunks[0], chunks[1], &app);
            draw_fft_chart(&mut f, chunks[2], &fft_app);
        })?;

        match events.next()? {
            Event::Input(input) => match &app.input_mode {
                InputMode::Normal => match input {
                    Key::Char('f') => {
                        app.input_mode = InputMode::CenterFreq;
                        events.disable_exit_key();
                    }
                    Key::Char('s') => {
                        app.input_mode = InputMode::SampleRate;
                        events.disable_exit_key();
                    }
                    Key::Char('b') => {
                        app.input_mode = InputMode::Bandwidth;
                        events.disable_exit_key();
                    }
                    Key::Char('q') => {
                        eprintln!("FFT UI in_count = {}", fft_app.in_count);
                        return Ok(());
                    }
                    _ => {}
                },
                _ => match input {
                    Key::Char('\n') => {
                        deal_with_input(fft_app, &app.input, &app.input_mode);
                        app.input.drain(..);
                        app.input_mode = InputMode::Normal;
                    }
                    Key::Char(c) => {
                        app.input.push(c);
                    }
                    Key::Backspace => {
                        app.input.pop();
                    }
                    Key::Esc => {
                        app.input_mode = InputMode::Normal;
                        events.enable_exit_key();
                    }
                    _ => {}
                },
            },
            Event::Tick => fft_app.update(),
        }
    }
}

fn deal_with_input(fft_app: &mut FFTApp, input: &str, mode: &InputMode) {
    match mode {
        InputMode::CenterFreq => {
            fft_app
                .controller
                .set_center_freq(input.parse::<u32>().unwrap())
                .unwrap();
        }
        InputMode::Bandwidth => {
            fft_app
                .controller
                .set_bandwidth(input.parse::<u32>().unwrap())
                .unwrap();
        }
        InputMode::SampleRate => {
            fft_app
                .controller
                .set_sample_rate(input.parse::<u32>().unwrap())
                .unwrap();
        }
        InputMode::Normal => (),
    }
}
