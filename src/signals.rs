use super::arguments;
use super::errors;
use super::http_signals_sender::HttpSignalSender;
use super::processes;
use super::unix_signals_sender::UnixSignalSender;
use mockall::automock;

// Sender is the trait implemented by the signal dispatcher. On its more simple form it is just a
// call to signal::kill(pid, sig) but it can get some extra implementations (e.g. do a http request
// to a different service). Implementations should be aware that send() is a blocker call from
// the oomhero loop perpective.
#[automock]
pub trait Sender {
    fn send(
        &self,
        severity: &arguments::CheckerResult,
        process: &processes::Process,
        cd: &processes::CollectedData,
    ) -> Result<(), errors::Error>;
}

pub enum SignalSenders {
    Unix(UnixSignalSender),
    Http(HttpSignalSender),
}

impl SignalSenders {
    // new creates the correct signal sender based on the provided command line flags. if a notify
    // command is provided then a Command instance is returned, if not then a Unix instance.
    pub fn new(flags: &arguments::Flags) -> Result<Self, errors::Error> {
        match &flags.http_file_path {
            None => {
                let sender = UnixSignalSender::new(flags.warning_signal, flags.critical_signal);
                Ok(SignalSenders::Unix(sender))
            }
            Some(path) => {
                let sender = HttpSignalSender::new(path.clone())?;
                Ok(SignalSenders::Http(sender))
            }
        }
    }
}

impl Sender for SignalSenders {
    // send is the Sender implementation for the enum. it forwards the call to the proper
    // implementation.
    fn send(
        &self,
        severity: &arguments::CheckerResult,
        process: &processes::Process,
        cd: &processes::CollectedData,
    ) -> Result<(), errors::Error> {
        match self {
            Self::Unix(sender) => sender.send(severity, process, cd),
            Self::Http(sender) => sender.send(severity, process, cd),
        }
    }
}
