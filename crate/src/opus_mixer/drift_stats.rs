/// Drift statistics for a single stream
#[derive(Debug)]
pub struct DriftStats {
    pub(crate) max_drift_seconds: f64,
    pub(crate) total_drift_seconds: f64,
    pub(crate) drift_samples: usize,
    pub(crate) max_compensation: f32,
    pub(crate) total_compensation: f32,
    pub(crate) compensation_samples: usize,
}

impl DriftStats {
    pub fn new() -> Self {
        Self {
            max_drift_seconds: 0.0,
            total_drift_seconds: 0.0,
            drift_samples: 0,
            max_compensation: 1.0,
            total_compensation: 0.0,
            compensation_samples: 0,
        }
    }

    pub fn update_drift(&mut self, drift_seconds: f64) {
        self.max_drift_seconds = self.max_drift_seconds.max(drift_seconds.abs());
        self.total_drift_seconds += drift_seconds.abs();
        self.drift_samples += 1;
    }

    pub fn update_compensation(&mut self, compensation: f32) {
        if (compensation - 1.0).abs() > 0.0001 {
            self.max_compensation = self.max_compensation.max((compensation - 1.0).abs() + 1.0);
            self.total_compensation += compensation;
            self.compensation_samples += 1;
        }
    }

    pub fn print_stats(&self) {
        if self.drift_samples > 0 {
            println!("  Maximum Drift: {:.3} ms", self.max_drift_seconds * 1000.0);
            println!(
                "  Average Drift: {:.3} ms",
                self.total_drift_seconds * 1000.0 / self.drift_samples as f64
            );
            println!("  Drift Measurements: {}", self.drift_samples);
        }
        if self.compensation_samples > 0 {
            println!(
                "  Maximum Speed Adjustment: {:.2}%",
                (self.max_compensation - 1.0) * 100.0
            );
            println!(
                "  Average Speed Adjustment: {:.2}%",
                ((self.total_compensation / self.compensation_samples as f32) - 1.0) * 100.0
            );
            println!(
                "  Compensation Applied: {} times",
                self.compensation_samples
            );
        }
    }
}
