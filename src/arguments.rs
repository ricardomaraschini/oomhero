use super::errors::Error;
use super::processes::CollectedData;
use clap::Parser;
use fasteval::Compiler;
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
        ThresholdsChecker::new_from(&self.warning, &self.critical)
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

// CheckerResult is the enum returned by a thresholds checker.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckerResult {
    Warning,
    Critical,
    None,
}

impl fmt::Display for CheckerResult {
    fn fmt(&self, fp: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let as_string = match self {
            CheckerResult::None => "none",
            CheckerResult::Warning => "warning",
            CheckerResult::Critical => "critical",
        };
        write!(fp, "{}", as_string)
    }
}

// ThresholdsChecker is an entity capable of evaluting a CollectedData struct against a warning and
// critical expressions. Both expressions are kept compiled into this struct for faster matching.
pub struct ThresholdsChecker {
    slab: fasteval::Slab,
    warning: fasteval::Instruction,
    critical: fasteval::Instruction,
}

impl ThresholdsChecker {
    // new_from parses the provided warning and critical expressions and if they are valid returns
    // a new ThresholdsChecker object holding the "compiled" version of both expressions.
    pub fn new_from(warning: &str, critical: &str) -> Result<Self, Error> {
        let mut slab = fasteval::Slab::new();

        let warning = fasteval::Parser::new()
            .parse(warning, &mut slab.ps)?
            .from(&slab.ps)
            .compile(&slab.ps, &mut slab.cs);
        let critical = fasteval::Parser::new()
            .parse(critical, &mut slab.ps)?
            .from(&slab.ps)
            .compile(&slab.ps, &mut slab.cs);

        Ok(Self {
            slab,
            warning,
            critical,
        })
    }

    // against checks the thresholds expressions against the provided collected data. expressions
    // are evaluated if they result in a value >= 1. they are considered a match. users should
    // leverage expressions that render into a boolean expression. For example: a "1" expression
    // will always evaluate to true while a "0" expression would evaluate to false. users can
    // do some math but they should use "==" (e.g. "memory_current + 10 == 100");
    pub fn against(&self, cd: &mut CollectedData) -> Result<CheckerResult, Error> {
        if self.critical.eval(&self.slab, cd)? >= 1. {
            return Ok(CheckerResult::Critical);
        }
        if self.warning.eval(&self.slab, cd)? >= 1. {
            return Ok(CheckerResult::Warning);
        }
        Ok(CheckerResult::None)
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

#[cfg(test)]
mod tests {
    use crate::processes;

    #[test]
    fn new_thresholds_checker() {
        assert_eq!(
            super::ThresholdsChecker::new_from(&String::from("0"), &String::from("0"),).is_ok(),
            true
        );
        assert_eq!(
            super::ThresholdsChecker::new_from(&String::from(">"), &String::from("0"),).is_ok(),
            false
        );
    }

    #[test]
    fn thresholds_checker_math() {
        let check = super::ThresholdsChecker::new_from(&String::from("1 - 1"), &String::from("1"))
            .expect("failed to create valid thresholds checker");

        let mut cd = processes::CollectedData {
            memory_max: 100,
            ..Default::default()
        };

        let result = check
            .against(&mut cd)
            .expect("failed to check against valid collect data");
        assert_eq!(result, super::CheckerResult::Critical);

        let check = super::ThresholdsChecker::new_from(
            &String::from("1 + 1 == 2"),
            &String::from("1 + 1 == 3"),
        )
        .expect("failed to create valid thresholds checker");

        let result = check
            .against(&mut cd)
            .expect("failed to check against valid collect data");
        assert_eq!(result, super::CheckerResult::Warning);
    }

    #[test]
    fn thresholds_checker_simple_evaluations() {
        let check = super::ThresholdsChecker::new_from(
            &String::from("memory_usage > 10"),
            &String::from("memory_usage > 20"),
        )
        .expect("failed to create valid thresholds checker");

        let mut cd = processes::CollectedData {
            memory_max: 100,
            ..Default::default()
        };

        let cases: [(u64, super::CheckerResult); 5] = [
            (0, super::CheckerResult::None),
            (9, super::CheckerResult::None),
            (11, super::CheckerResult::Warning),
            (21, super::CheckerResult::Critical),
            (9, super::CheckerResult::None),
        ];
        for (current, expected) in cases {
            cd.memory_current = current;
            let result = check
                .against(&mut cd)
                .expect("failed to check against valid collect data");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn thresholds_checker_combined_evaluations() {
        let check = super::ThresholdsChecker::new_from(
            &String::from("memory_usage > 10 && memory_pressure_full_avg10 > 60"),
            &String::from("memory_usage > 20 && memory_pressure_full_avg10 > 70"),
        )
        .expect("failed to create valid thresholds checker");

        let mut cd = processes::CollectedData {
            memory_max: 100,
            ..Default::default()
        };

        let cases = vec![
            (0, 0., super::CheckerResult::None),
            (11, 0., super::CheckerResult::None),
            (11, 100., super::CheckerResult::Warning),
            (21, 100., super::CheckerResult::Critical),
            (0, 100., super::CheckerResult::None),
        ];
        for (current, pressure, expected) in cases {
            cd.memory_current = current;
            cd.pressure.memory.full.avg10 = pressure;
            let result = check
                .against(&mut cd)
                .expect("failed to check against valid collect data");
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn thresholds_checker_different_counters() {
        let check = super::ThresholdsChecker::new_from(
            &String::from("memory_usage > 70"),
            &String::from("memory_pressure_full_avg10 > 20"),
        )
        .expect("failed to create valid thresholds checker");

        let mut cd = processes::CollectedData {
            memory_max: 100,
            ..Default::default()
        };

        let cases = vec![
            (0, 0., super::CheckerResult::None),
            (70, 20., super::CheckerResult::None),
            (71, 20., super::CheckerResult::Warning),
            (100, 20., super::CheckerResult::Warning),
            (0, 21., super::CheckerResult::Critical),
        ];
        for (current, pressure, expected) in cases {
            cd.memory_current = current;
            cd.pressure.memory.full.avg10 = pressure;
            let result = check
                .against(&mut cd)
                .expect("failed to check against valid collect data");
            assert_eq!(result, expected);
        }
    }
}
