use super::arguments;
use super::errors;
use super::processes;
use super::signals;
use nix::sys::signal;
use nix::unistd;

// UnixSignalSender is the entity that sends a unix signal to a given process.
#[derive(Debug)]
pub struct UnixSignalSender {
    pub warning: signal::Signal,
    pub critical: signal::Signal,
}

impl UnixSignalSender {
    pub fn new(warning: signal::Signal, critical: signal::Signal) -> Self {
        Self { warning, critical }
    }
}

impl Default for UnixSignalSender {
    fn default() -> Self {
        Self {
            warning: signal::SIGUSR1,
            critical: signal::SIGUSR2,
        }
    }
}

impl signals::Sender for UnixSignalSender {
    // send sends a signal to a process pointed by pid. the signal sent depends on the severity
    // that can be either critical or warning (if none then the signal is just ignored).
    fn send(
        &self,
        severity: &arguments::CheckerResult,
        process: &processes::Process,
        _: &processes::CollectedData,
    ) -> Result<(), errors::Error> {
        let sig = match severity {
            arguments::CheckerResult::None => return Ok(()),
            arguments::CheckerResult::Warning => self.warning,
            arguments::CheckerResult::Critical => self.critical,
        };
        let pid = unistd::Pid::from_raw(process.pid);
        Ok(signal::kill(pid, sig)?)
    }
}
