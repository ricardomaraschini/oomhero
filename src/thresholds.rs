use super::errors::Error;
use super::processes;
use clap::Parser;
use clap::ValueEnum;

// StallSeverity holds both severities as presented by the kernel on a pressure file.
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum StallSeverity {
    #[default]
    Some,
    Full,
}

// StallWindow holds all windows across the kernel keeps track of a resource pressure.
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum StallWindow {
    #[default]
    Avg10,
    Avg60,
    Avg300,
}

// UserProvided holds all thresholds supported by the monitor that can be customized by the user.
// this struct is tailored to be used with the clap crate (allows for user provided data).
#[derive(Parser, Clone, Debug, Default)]
pub struct UserProvided {
    #[arg(
        long,
        default_value = "0",
        requires = "memory_usage_critical",
        help = "Warning watermark for memory usage (in percentage)"
    )]
    pub memory_usage_warning: i32,

    #[arg(
        long,
        default_value = "0",
        requires = "memory_usage_warning",
        help = "Critical watermark for memory usage (in percentage)"
    )]
    pub memory_usage_critical: i32,

    #[arg(
        long,
        default_value = "0",
        requires = "memory_pressure_critical",
        help = "Warning watermark for memory pressure (in percentage)"
    )]
    pub memory_pressure_warning: i32,

    #[arg(
        long,
        default_value = "0",
        requires = "memory_pressure_warning",
        help = "Critical watermark for memory pressure (in percentage)"
    )]
    pub memory_pressure_critical: i32,

    #[arg(
        long,
        default_value = "0",
        requires = "io_pressure_critical",
        help = "Warning watermark for io pressure (in percentage)"
    )]
    pub io_pressure_warning: i32,

    #[arg(
        long,
        default_value = "0",
        requires = "io_pressure_warning",
        help = "Critical watermark for io pressure (in percentage)"
    )]
    pub io_pressure_critical: i32,

    #[arg(
        long,
        default_value = "0",
        requires = "cpu_pressure_critical",
        help = "Warning watermark for cpu pressure (in percentage)"
    )]
    pub cpu_pressure_warning: i32,

    #[arg(
        long,
        default_value = "0",
        requires = "cpu_pressure_warning",
        help = "Critical watermark for cpu pressure (in percentage)"
    )]
    pub cpu_pressure_critical: i32,

    #[arg(
        long,
        default_value = "full",
        help = "The stall severity level to use, see kernel documentation for details"
    )]
    pub stall_severity: StallSeverity,

    #[arg(
        long,
        default_value = "avg10",
        help = "The stall window to use, see kernel documentation for details"
    )]
    pub stall_window: StallWindow,
}

impl UserProvided {
    // has_memory_usage_threholds returns true if warning and critical thresholds are set for
    // memory usage.
    pub fn has_memory_usage_threholds(&self) -> bool {
        self.memory_usage_warning > 0 && self.memory_usage_critical > 0
    }

    // has_memory_pressure_thresholds returns true if warning and critical thresholds are set
    // for memory pressure.
    pub fn has_memory_pressure_thresholds(&self) -> bool {
        self.memory_pressure_warning > 0 && self.memory_pressure_critical > 0
    }

    // has_io_pressure_thresholds returns true if we have warning and critical for io pressure.
    pub fn has_io_pressure_thresholds(&self) -> bool {
        self.io_pressure_warning > 0 && self.io_pressure_critical > 0
    }

    // has_cpu_pressure_thresholds returns true if we have warning and critical for cpu pressure.
    pub fn has_cpu_pressure_thresholds(&self) -> bool {
        self.cpu_pressure_warning > 0 && self.cpu_pressure_critical > 0
    }

    // validate verifies we have warning and critical for at least one of out counters: memory
    // usage, memory pressure, io pressure, and cpu pressure.
    pub fn validate(&self) -> Result<(), Error> {
        if self.has_memory_usage_threholds() {
            return Ok(());
        }
        if self.has_memory_pressure_thresholds() {
            return Ok(());
        }
        if self.has_io_pressure_thresholds() {
            return Ok(());
        }
        if self.has_cpu_pressure_thresholds() {
            return Ok(());
        }
        return Err(Error::Message(format!(
            "missing warning and critical for at least one specific counter"
        )));
    }

    // select_pressure_value_to_compare returns the right value we must use to compare a pressure
    // against its watermarks. this depends on what has been selected on both stall_severity and
    // stall_window.
    fn select_pressure_value_to_compare(&self, pressure_data: &processes::PressureData) -> f32 {
        let mut stall_severity = &pressure_data.full;
        if let StallSeverity::Some = self.stall_severity {
            stall_severity = &pressure_data.some;
        }
        match self.stall_window {
            StallWindow::Avg10 => stall_severity.avg10,
            StallWindow::Avg60 => stall_severity.avg60,
            StallWindow::Avg300 => stall_severity.avg300,
        }
    }

    // check_against checks the thresholds against the collected data. returns a tuple of bool
    // where .0 is warning and .1 is critical.
    pub fn check_against(&self, cd: &processes::CollectedData) -> (bool, bool) {
        let mut warning = false;
        let mut critical = false;

        if self.has_memory_usage_threholds() {
            if cd.memory_usage() > self.memory_usage_critical as f32 {
                critical = true;
            } else if cd.memory_usage() > self.memory_usage_warning as f32 {
                warning = true;
            }
        }

        if self.has_memory_pressure_thresholds() {
            let collected_value = self.select_pressure_value_to_compare(&cd.pressure.memory);
            if collected_value > self.memory_pressure_critical as f32 {
                critical = true;
            } else if collected_value > self.memory_pressure_warning as f32 {
                warning = true;
            }
        }

        if self.has_io_pressure_thresholds() {
            let collected_value = self.select_pressure_value_to_compare(&cd.pressure.io);
            if collected_value > self.io_pressure_critical as f32 {
                critical = true;
            } else if collected_value > self.io_pressure_warning as f32 {
                warning = true;
            }
        }

        if self.has_cpu_pressure_thresholds() {
            let collected_value = self.select_pressure_value_to_compare(&cd.pressure.cpu);
            if collected_value > self.cpu_pressure_critical as f32 {
                critical = true;
            } else if collected_value > self.cpu_pressure_warning as f32 {
                warning = true;
            }
        }

        (warning, critical)
    }
}
