use log::warn;
use std::cmp::Ordering;
use std::sync::mpsc;

// Event is a struct used to represent an event on the system. events are more usually than not
// related to a pid. for example: upon reading the memory usage for pid X an event may be sent
// with the following format: Event{pid: X, message: "permission denied"}.
#[derive(Debug, Clone)]
pub struct Event {
    pub pid: i32,
    pub message: String,
    pub priority: Priority,
    pub memory_usage: f32,
    pub cmdline: String,
}

// Priority determines how relevant an event on the system is. receivers of such events should
// choose how to deal with them.
#[derive(Debug, Clone)]
pub enum Priority {
    Low,
    High,
}

impl Event {
    // new returns a new event with what are considered sane defaults set.
    pub fn default() -> Self {
        Event {
            pid: 0,
            message: String::new(),
            priority: Priority::Low,
            memory_usage: 0.0,
            cmdline: String::new(),
        }
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

    // with_cmdline sets the cmdline property of an event.
    pub fn with_cmdline(mut self, cmdline: String) -> Self {
        self.cmdline = cmdline;
        self
    }

    // deviates_significantly indicates if the current event differs enough to deserve to be acted
    // upon. for example: if the usage differs more than 10% we return true. used on the system to
    // determine if an event deserves to be printed.
    pub fn deviates_significantly(&self, from: &Event) -> bool {
        if (self.memory_usage - from.memory_usage).abs() > 10_f32 {
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
