//! btop++ style widgets: animated braille wave, gradient meters, and micro-charts.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

const BRAILLE_CHARS: [char; 8] = ['⣀', '⣄', '⣤', '⣦', '⣶', '⣷', '⣿', '█'];
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct WaveGenerator {
    phase: f64,
    history: Vec<f64>,
}

impl WaveGenerator {
    pub fn new(capacity: usize) -> Self {
        Self {
            phase: 0.0,
            history: vec![0.0; capacity],
        }
    }

    pub fn tick(&mut self, active_signal: bool) {
        self.phase += 0.25;
        let base_amp = if active_signal { 0.85 } else { 0.35 };
        let new_val = (self.phase.sin() * 0.5 + 0.5) * base_amp
            + (self.phase * 0.3).cos() * 0.15;
        let clamped = new_val.clamp(0.05, 0.98);

        self.history.remove(0);
        self.history.push(clamped);
    }

    pub fn render_lines(&self, width: usize, color: Color) -> Vec<Line<'static>> {
        let count = width.min(self.history.len());
        let slice = &self.history[self.history.len() - count..];

        // 2-row braille waveform
        let mut row_upper = String::with_capacity(count);
        let mut row_lower = String::with_capacity(count);

        for &val in slice {
            let height_idx = ((val * 7.9) as usize).min(7);
            let char_repr = BRAILLE_CHARS[height_idx];

            if val > 0.5 {
                row_upper.push(char_repr);
                row_lower.push('⣿');
            } else {
                row_upper.push(' ');
                row_lower.push(char_repr);
            }
        }

        vec![
            Line::from(vec![Span::styled(row_upper, Style::default().fg(color))]),
            Line::from(vec![Span::styled(row_lower, Style::default().fg(color))]),
        ]
    }
}

pub fn render_gradient_meter(ratio: f64, width: usize) -> Vec<Span<'static>> {
    let filled = ((ratio.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    let empty = width.saturating_sub(filled);

    let mut spans = Vec::new();

    // Gradient calculation
    for i in 0..filled {
        let t = i as f64 / width as f64;
        let (r, g, b) = interpolate_color(t);
        spans.push(Span::styled("█", Style::default().fg(Color::Rgb(r, g, b))));
    }

    if empty > 0 {
        spans.push(Span::styled(
            "░".repeat(empty),
            Style::default().fg(Color::Rgb(50, 55, 70)),
        ));
    }

    spans
}

fn interpolate_color(t: f64) -> (u8, u8, u8) {
    // Gradient: Cyan (139, 233, 253) -> Blue (92, 156, 245) -> Green (80, 250, 123)
    if t < 0.5 {
        let factor = t * 2.0;
        let r = (139.0 * (1.0 - factor) + 92.0 * factor) as u8;
        let g = (233.0 * (1.0 - factor) + 156.0 * factor) as u8;
        let b = (253.0 * (1.0 - factor) + 245.0 * factor) as u8;
        (r, g, b)
    } else {
        let factor = (t - 0.5) * 2.0;
        let r = (92.0 * (1.0 - factor) + 80.0 * factor) as u8;
        let g = (156.0 * (1.0 - factor) + 250.0 * factor) as u8;
        let b = (245.0 * (1.0 - factor) + 123.0 * factor) as u8;
        (r, g, b)
    }
}

pub fn get_spinner(tick: usize) -> &'static str {
    SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]
}
