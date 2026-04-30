use super::processes;
use log::warn;
use std::cmp::Ordering;
use std::sync::mpsc;

// Event is a struct used to represent an event on the system. events are more usually than not
// related to a pid. for example: upon reading the memory usage for pid X an event may be sent
// with the following format: Event{pid: X, message: "permission denied"}.
#[derive(Debug, Clone, Default)]
pub struct Event {
    pub pid: i32,
    pub message: String,
    pub priority: Priority,
    pub cmdline: String,
    pub memory_usage: f32,
    pub memory_pressure: f32,
    pub io_pressure: f32,
    pub cpu_pressure: f32,
}

// Priority determines how relevant an event on the system is. receivers of such events should
// choose how to deal with them.
#[derive(Debug, Clone, Default)]
pub enum Priority {
    #[default]
    Low,
    High,
}

impl Event {
    // low_prio returns a new event with low priority set, this is a sugar coating on top of
    // default as it already returns a low priority event. we overwrite so if the default
    // changes in the future we still have this one.
    pub fn low_prio() -> Self {
        let mut event = Event::default();
        event.priority = Priority::Low;
        event
    }

    // high_prio returns a default event with the priority set to high.
    pub fn high_prio() -> Self {
        let mut event = Event::default();
        event.priority = Priority::High;
        event
    }

    // with_pid sets the pid for the event.
    pub fn with_pid(mut self, pid: i32) -> Self {
        self.pid = pid;
        self
    }

    // with_message sets the event message.
    pub fn with_message(mut self, message: String) -> Self {
        self.message = message;
        self
    }

    // with_priority sets the event priority.
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    // with_memory_usage sets the memory usage property inside the event.
    pub fn with_memory_usage(mut self, usage: f32) -> Self {
        self.memory_usage = usage;
        self
    }

    // with_memory_pressure sets the memory pressure counter inside the event.
    pub fn with_memory_pressure(mut self, pressure: f32) -> Self {
        self.memory_pressure = pressure;
        self
    }

    // with_io_pressure sets the io pressure counter inside the event.
    pub fn with_io_pressure(mut self, pressure: f32) -> Self {
        self.io_pressure = pressure;
        self
    }

    // with_cpu_pressure sets the cpu pressure counter inside the event.
    pub fn with_cpu_pressure(mut self, pressure: f32) -> Self {
        self.cpu_pressure = pressure;
        self
    }

    // with_cmdline sets the cmdline property of an event.
    pub fn with_cmdline(mut self, cmdline: String) -> Self {
        self.cmdline = cmdline;
        self
    }

    // with_collected_data adds all collected data struct to the event.
    pub fn with_collected_data(self, cd: &processes::CollectedData) -> Self {
        self.with_memory_usage(cd.memory_usage())
            .with_memory_pressure(cd.pressure.memory.full.avg10)
            .with_io_pressure(cd.pressure.io.full.avg10)
            .with_cpu_pressure(cd.pressure.cpu.full.avg10)
    }

    // with_process incorporates information about the provided process to an event.
    pub fn with_process(self, process: &processes::Process) -> Self {
        self.with_pid(process.pid)
            .with_cmdline(process.cmdline.clone())
    }

    // with_process_collected_data adds information about the process and the collected data
    // provided as arguments to the body of an event.
    pub fn with_process_collected_data(
        self,
        process: &processes::Process,
        cd: &processes::CollectedData,
    ) -> Self {
        self.with_process(process).with_collected_data(cd)
    }

    // deviates_significantly indicates if the current event differs enough to deserve to be acted
    // upon. for example: if the usage differs more than 10% we return true. used on the system to
    // determine if an event deserves to be logged.
    pub fn deviates_significantly(&self, from: &Event) -> bool {
        let max = 10_f32;
        if (self.memory_usage - from.memory_usage).abs() > max {
            return true;
        }
        if (self.memory_pressure - from.memory_pressure).abs() > max {
            return true;
        }
        if (self.io_pressure - from.io_pressure).abs() > max {
            return true;
        }
        if (self.cpu_pressure - from.cpu_pressure).abs() > max {
            return true;
        }
        if let Ordering::Equal = self.message.cmp(&from.message) {
            return false;
        }
        true
    }
}

// Transmitter is an entity capable of transmitting events.
pub struct Transmitter {
    channel: mpsc::Sender<Event>,
}

impl Transmitter {
    // new returns a new transmitter capable of transmitting events through the provided channel.
    pub fn new(channel: mpsc::Sender<Event>) -> Self {
        Transmitter { channel }
    }

    // send sends the event through the provided Sender. in case of failure the error seen
    // at transmission time is logged. we lose the original event but we have a bigger fish
    // to fry if we ever encounter this.
    pub fn send(&self, event: Event) {
        if let Err(err) = self.channel.send(event) {
            warn!("error sending message: {err}");
        }
    }
}
