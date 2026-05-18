use super::errors::Error;
use super::processes;
use clap::Parser;
use clap::ValueEnum;
use fasteval;
use nix::sys::signal;
use std::fmt;
use std::time;

const ABOUT: &str = "
   ,.   (   .      )        .
  ('     )  )'     ,'
.; )  '▌(( (' )    ;(,  ,' ((
▛▌▛▌▛▛▌▛▌█▌▛▘▛▌(..,( . )_  _'
▙▌▙▌▌▌▌▌▌▙▖▌ ▙▌

A lightweight Kubernetes sidecar that monitors process resource usage and pressure metrics, sending
configurable signals to applications before resource exhaustion occurs. OOMHero runs alongside your
application containers in Kubernetes pods, continuously monitoring memory usage, memory pressure,
I/O pressure, and CPU pressure. When processes approach configurable thresholds, OOMHero sends Unix
signals to enable proactive remediation before the OOMKiller terminates your application.

For Kernel pressure information please visit https://docs.kernel.org/accounting/psi.html.
";

#[derive(Parser, Debug)]
#[command(name = "oomhero")]
#[command(about = ABOUT)]
pub struct Flags {
    #[arg(
        long,
        default_value = "100ms",
        value_parser = parse_duration,
        help = "Interval to be used as a pause between process scans"
    )]
    pub loop_interval: time::Duration,

    #[arg(
        long,
        default_value = "30s",
        value_parser = parse_duration,
        help = "Interval to wait before sending the same signal to the same process"
    )]
    pub cooldown_interval: time::Duration,

    #[arg(
        long,
        default_value = "SIGUSR1",
        help = "Signal to be send when a process crosses the warning watermark"
    )]
    pub warning_signal: signal::Signal,

    #[arg(
        long,
        default_value = "SIGUSR2",
        help = "Signal to be send when a process crosses the critical watermark"
    )]
    pub critical_signal: signal::Signal,

    #[arg(long, default_value = "false", help = "Print version")]
    pub version: bool,

    #[command(flatten)]
    pub thresholds: Thresholds,
}

impl fmt::Display for Flags {
    fn fmt(&self, fp: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(fp, "loop:{:?} ", self.loop_interval)?;
        write!(fp, "cooldown:{:?} ", self.cooldown_interval)?;
        write!(
            fp,
            "signals:{:?},{:?} ",
            self.warning_signal, self.critical_signal
        )?;
        write!(fp, "{}", self.thresholds)?;
        Ok(())
    }
}

// parse_duration is used to parse the interval command line flag.
fn parse_duration(s: &str) -> Result<time::Duration, String> {
    duration_str::parse(s).map_err(|e| e.to_string())
}

// validate_expression attempts to parse the expression provided by the user and errors out if it
// does not make sense to us. This is just for early return.
fn validate_expression(s: &str) -> Result<String, String> {
    let mut slab = fasteval::Slab::new();
    let parser = fasteval::Parser::new();
    match parser.parse(s, &mut slab.ps) {
        Ok(_) => Ok(s.to_owned()),
        Err(err) => Err(format!("invalid expression: {}", err)),
    }
}

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

// Thresholds holds all thresholds supported by the monitor that can be customized by the user.
// This struct is tailored to be used with the clap crate (allows for user provided data).
#[derive(Parser, Clone, Debug)]
pub struct Thresholds {
    #[arg(
        long,
        requires = "memory_usage_critical",
        help = "Warning watermark for memory usage (in percentage)"
    )]
    pub memory_usage_warning: Option<i32>,

    #[arg(
        long,
        requires = "memory_usage_warning",
        help = "Critical watermark for memory usage (in percentage)"
    )]
    pub memory_usage_critical: Option<i32>,

    #[arg(
        long,
        requires = "memory_pressure_critical",
        help = "Warning watermark for memory pressure (in percentage)"
    )]
    pub memory_pressure_warning: Option<i32>,

    #[arg(
        long,
        requires = "memory_pressure_warning",
        help = "Critical watermark for memory pressure (in percentage)"
    )]
    pub memory_pressure_critical: Option<i32>,

    #[arg(
        long,
        requires = "io_pressure_critical",
        help = "Warning watermark for io pressure (in percentage)"
    )]
    pub io_pressure_warning: Option<i32>,

    #[arg(
        long,
        requires = "io_pressure_warning",
        help = "Critical watermark for io pressure (in percentage)"
    )]
    pub io_pressure_critical: Option<i32>,

    #[arg(
        long,
        requires = "cpu_pressure_critical",
        help = "Warning watermark for cpu pressure (in percentage)"
    )]
    pub cpu_pressure_warning: Option<i32>,

    #[arg(
        long,
        requires = "cpu_pressure_warning",
        help = "Critical watermark for cpu pressure (in percentage)"
    )]
    pub cpu_pressure_critical: Option<i32>,

    #[arg(long, default_value = "full", help = "The stall severity level to use")]
    pub stall_severity: StallSeverity,

    #[arg(long, default_value = "avg10", help = "The stall window to use")]
    pub stall_window: StallWindow,

    #[arg(
        long,
        requires = "critical_expression",
        value_parser = validate_expression,
        help = "Expression whose evaluation causes a warning signal"
    )]
    pub warning_expression: Option<String>,

    #[arg(
        long,
        requires = "warning_expression",
        value_parser = validate_expression,
        help = "Expression whose evaluation causes a critical signal"
    )]
    pub critical_expression: Option<String>,
}

impl Thresholds {
    // has_memory_usage_threholds returns true if warning and critical thresholds are set for
    // memory usage.
    fn has_memory_usage_threholds(&self) -> bool {
        self.memory_usage_warning.is_some() && self.memory_usage_critical.is_some()
    }

    // has_memory_pressure_thresholds returns true if warning and critical thresholds are set
    // for memory pressure.
    fn has_memory_pressure_thresholds(&self) -> bool {
        self.memory_pressure_warning.is_some() && self.memory_pressure_critical.is_some()
    }

    // has_io_pressure_thresholds returns true if we have warning and critical for io pressure.
    fn has_io_pressure_thresholds(&self) -> bool {
        self.io_pressure_warning.is_some() && self.io_pressure_critical.is_some()
    }

    // has_cpu_pressure_thresholds returns true if we have warning and critical for cpu pressure.
    fn has_cpu_pressure_thresholds(&self) -> bool {
        self.cpu_pressure_warning.is_some() && self.cpu_pressure_critical.is_some()
    }

    // has_expression_thresholds evalutes is expressions have been provided for both warning and
    // critical thresholds.
    fn has_expression_thresholds(&self) -> bool {
        self.warning_expression.is_some() && self.critical_expression.is_some()
    }

    // validate verifies we have warning and critical for at least one of our counters: memory
    // usage, memory pressure, io pressure, and cpu pressure.
    pub fn validate(&self) -> Result<(), Error> {
        if self.has_memory_usage_threholds()
            || self.has_memory_pressure_thresholds()
            || self.has_io_pressure_thresholds()
            || self.has_cpu_pressure_thresholds()
            || self.has_expression_thresholds()
        {
            Ok(())
        } else {
            Err(Error::Message(
                "missing warning and critical for at least one specific counter".to_string(),
            ))
        }
    }

    // select_pressure_value_to_compare returns the right value we must use to compare a pressure
    // against its watermarks. This depends on what has been selected on both stall_severity and
    // stall_window.
    fn select_pressure_value_to_compare(&self, pressure_data: &processes::PressureData) -> f32 {
        let stall_severity = match self.stall_severity {
            StallSeverity::Some => &pressure_data.some,
            StallSeverity::Full => &pressure_data.full,
        };
        match self.stall_window {
            StallWindow::Avg10 => stall_severity.avg10,
            StallWindow::Avg60 => stall_severity.avg60,
            StallWindow::Avg300 => stall_severity.avg300,
        }
    }

    // check_against checks the thresholds against the collected data. Returns a tuple of bool
    // where .0 is warning and .1 is critical.
    pub fn check_against(&self, cd: &processes::CollectedData) -> (bool, bool) {
        let mut warning = false;
        let mut critical = false;

        if let (Some(w), Some(c)) = (self.memory_usage_warning, self.memory_usage_critical) {
            if cd.memory_usage() > c as f32 {
                critical = true;
            } else if cd.memory_usage() > w as f32 {
                warning = true;
            }
        }

        if let (Some(w), Some(c)) = (self.memory_pressure_warning, self.memory_pressure_critical) {
            let collected_value = self.select_pressure_value_to_compare(&cd.pressure.memory);
            if collected_value > c as f32 {
                critical = true;
            } else if collected_value > w as f32 {
                warning = true;
            }
        }

        if let (Some(w), Some(c)) = (self.io_pressure_warning, self.io_pressure_critical) {
            let collected_value = self.select_pressure_value_to_compare(&cd.pressure.io);
            if collected_value > c as f32 {
                critical = true;
            } else if collected_value > w as f32 {
                warning = true;
            }
        }

        if let (Some(w), Some(c)) = (self.cpu_pressure_warning, self.cpu_pressure_critical) {
            let collected_value = self.select_pressure_value_to_compare(&cd.pressure.cpu);
            if collected_value > c as f32 {
                critical = true;
            } else if collected_value > w as f32 {
                warning = true;
            }
        }

        if self.has_expression_thresholds() {}

        (warning, critical)
    }
}

impl fmt::Display for Thresholds {
    fn fmt(&self, fp: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        if let (Some(w), Some(c)) = (self.memory_usage_warning, self.memory_usage_critical) {
            write!(fp, "memory_usage:{},{} ", w, c)?;
        }

        let mut has_pressure = false;
        if let (Some(w), Some(c)) = (self.memory_pressure_warning, self.memory_pressure_critical) {
            has_pressure = true;
            write!(fp, "memory_pressure:{},{} ", w, c)?;
        }

        if let (Some(w), Some(c)) = (self.io_pressure_warning, self.io_pressure_critical) {
            has_pressure = true;
            write!(fp, "io_pressure:{},{} ", w, c,)?;
        }

        if let (Some(w), Some(c)) = (self.cpu_pressure_warning, self.cpu_pressure_critical) {
            has_pressure = true;
            write!(fp, "cpu_pressure:{},{} ", w, c)?;
        }

        if has_pressure {
            write!(fp, "stall_severity:{:?} ", self.stall_severity)?;
            write!(fp, "stall_window:{:?} ", self.stall_window)?;
        }

        if let (Some(w), Some(c)) = (&self.warning_expression, &self.critical_expression) {
            write!(fp, "expressions:'{}','{}' ", w, c)?;
        }

        Ok(())
    }
}
