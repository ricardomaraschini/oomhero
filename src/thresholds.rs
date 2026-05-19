use super::errors;
use super::processes;
use clap::Parser;
use fasteval::Evaler;
use std::fmt;

// Thresholds holds all thresholds supported by the monitor that can be customized by the user.
// This struct is tailored to be used with the clap crate (allows for user provided data).
#[derive(Parser, Debug)]
pub struct Thresholds {
    #[arg(
        long,
        value_parser = validate_expression,
        help = "Expression whose evaluation causes a warning signal"
    )]
    pub warning: String,

    #[arg(
        long,
        value_parser = validate_expression,
        help = "Expression whose evaluation causes a critical signal"
    )]
    pub critical: String,
}

impl Thresholds {
    // checker "compiles" both warning and critical expressions and return an entity capable of
    // being assessed against collected values.
    pub fn checker(&self) -> Result<Checker, errors::Error> {
        let parser = fasteval::Parser::new();

        let mut warning_slab = fasteval::Slab::new();
        let warning = parser.parse(&self.warning, &mut warning_slab.ps)?;

        let mut critical_slab = fasteval::Slab::new();
        let critical = parser.parse(&self.critical, &mut critical_slab.ps)?;

        Ok(Checker {
            warning_slab,
            warning,
            critical_slab,
            critical,
        })
    }
}

impl fmt::Display for Thresholds {
    fn fmt(&self, fp: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(fp, "w:'{}', c:'{}' ", self.warning, self.critical)
    }
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

// Checker is an entity capable of evaluting a CollectedData struct against a warning and critical
// expressions.
pub struct Checker {
    warning_slab: fasteval::Slab,
    critical_slab: fasteval::Slab,
    warning: fasteval::parser::ExpressionI,
    critical: fasteval::parser::ExpressionI,
}

impl Checker {
    // against checks the thresholds expressions against the provided collected data. Returns a
    // tuple of bool where .0 is warning and .1 is critical.
    pub fn against(
        &self,
        cd: &mut processes::CollectedData,
    ) -> Result<(bool, bool), errors::Error> {
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
