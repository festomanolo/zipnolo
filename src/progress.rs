use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Thread-safe terminal progress bar using lock-free atomics
pub struct ProgressBar {
    label: String,
    total_bytes: u64,
    processed_bytes: AtomicU64,
    last_render_ns: AtomicU64,
    start_time: Instant,
    enabled: bool,
}

impl ProgressBar {
    pub fn new(label: impl Into<String>, total_bytes: u64, enabled: bool) -> Self {
        let pb = Self {
            label: label.into(),
            total_bytes,
            processed_bytes: AtomicU64::new(0),
            last_render_ns: AtomicU64::new(0),
            start_time: Instant::now(),
            enabled,
        };
        if pb.enabled {
            pb.render(0, true);
        }
        pb
    }

    pub fn add(&self, bytes: u64) {
        let current = self.processed_bytes.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if !self.enabled {
            return;
        }
        let elapsed_ns = self.start_time.elapsed().as_nanos() as u64;
        let prev_ns = self.last_render_ns.load(Ordering::Relaxed);
        // Throttle terminal updates to ~25 FPS unless finished
        if current >= self.total_bytes || elapsed_ns.saturating_sub(prev_ns) >= 40_000_000 {
            if self
                .last_render_ns
                .compare_exchange(prev_ns, elapsed_ns, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
                || current >= self.total_bytes
            {
                self.render(current, false);
            }
        }
    }

    pub fn finish(&self) {
        if !self.enabled {
            return;
        }
        let final_bytes = self
            .processed_bytes
            .load(Ordering::Relaxed)
            .max(self.total_bytes);
        self.render(final_bytes, true);
        eprintln!();
    }

    fn render(&self, current: u64, force: bool) {
        let total = self.total_bytes;
        let ratio = if total == 0 {
            1.0
        } else {
            (current as f64 / total as f64).clamp(0.0, 1.0)
        };
        let width = 28usize;
        let filled = (ratio * width as f64).round() as usize;
        let empty = width.saturating_sub(filled);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

        let elapsed = self.start_time.elapsed().as_secs_f64();
        let speed_bps = if elapsed > 0.000_1 {
            current as f64 / elapsed
        } else {
            0.0
        };
        let eta_secs = if speed_bps > 0.0 && current < total {
            (total - current) as f64 / speed_bps
        } else {
            0.0
        };

        if force || current <= total {
            eprint!(
                "\r{:<13} [{}] {:>5.1}% | {} / {} | {}/s | ETA {:.1}s   ",
                self.label,
                bar,
                ratio * 100.0,
                format_bytes(current),
                format_bytes(total),
                format_bytes(speed_bps as u64),
                eta_secs
            );
            let _ = io::stderr().flush();
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}
