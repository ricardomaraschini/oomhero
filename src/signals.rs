use super::errors;
use mockall::automock;
use nix::sys::signal;
use nix::unistd;

// Sender is the trait implemented by the signal sender. We have this mostly for testing purposes
// as we only have one signal sender anyways.
#[automock]
pub trait Sender {
    fn send(&self, sig: signal::Signal, pid: i32) -> Result<(), errors::Error>;
}

// SignalSender is the entity that sends a unix signal to a given process.
#[derive(Debug, Default)]
pub struct SignalSender {}

impl Sender for SignalSender {
    // send sends a signal to a process pointed by pid.
    fn send(&self, sig: signal::Signal, pid: i32) -> Result<(), errors::Error> {
        let pid = unistd::Pid::from_raw(pid);
        Ok(signal::kill(pid, sig)?)
    }
}
