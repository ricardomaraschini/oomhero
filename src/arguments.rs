use super::thresholds;
use clap::Parser;
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
    pub thresholds: thresholds::Thresholds,
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
