use super::errors::Error;
use super::processes::CollectedData;
use clap::Parser;
use fasteval::Evaler;
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
#[command(name = "oomhero", about = ABOUT)]
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

    #[arg(
        long,
        default_value = "memory_usage > 70",
        value_parser = validate_expression,
        help = "Expression whose evaluation causes a warning signal"
    )]
    pub warning: String,

    #[arg(
        long,
        default_value = "memory_usage > 80",
        value_parser = validate_expression,
        help = "Expression whose evaluation causes a critical signal"
    )]
    pub critical: String,
}

impl Flags {
    // thresholds_checker "compiles" both warning and critical expressions and return an entity
    // capable of being assessed against processes::CollectedData.
    pub fn thresholds_checker(&self) -> Result<ThresholdsChecker, Error> {
        let parser = fasteval::Parser::new();

        let mut warning_slab = fasteval::Slab::new();
        let warning = parser.parse(&self.warning, &mut warning_slab.ps)?;

        let mut critical_slab = fasteval::Slab::new();
        let critical = parser.parse(&self.critical, &mut critical_slab.ps)?;

        Ok(ThresholdsChecker {
            warning_slab,
            critical_slab,
            warning,
            critical,
        })
    }
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
        write!(
            fp,
            "warning:'{}', critical:'{}'",
            self.warning, self.critical
        )
    }
}

// ThresholdsChecker is an entity capable of evaluting a CollectedData struct against a warning and
// critical expressions.
pub struct ThresholdsChecker {
    warning_slab: fasteval::Slab,
    critical_slab: fasteval::Slab,
    warning: fasteval::parser::ExpressionI,
    critical: fasteval::parser::ExpressionI,
}

impl ThresholdsChecker {
    // against checks the thresholds expressions against the provided collected data. Returns a
    // tuple of bool where .0 is warning and .1 is critical.
    pub fn against(&self, cd: &mut CollectedData) -> Result<(bool, bool), Error> {
        let warning = self
            .warning
            .from(&self.warning_slab.ps)
            .eval(&self.warning_slab, cd)?;

        let critical = self
            .critical
            .from(&self.critical_slab.ps)
            .eval(&self.critical_slab, cd)?;

        Ok((warning >= 1., critical >= 1.))
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
