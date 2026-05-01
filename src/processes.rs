use super::cgroups;
use super::errors::Error;
use log::debug;
use nix::sys::signal;
use nix::unistd;
use std::fs;
use std::io;
use std::io::BufRead;
use std::str;

// Pressure gathers all the pressure data we read for each single process. We collect pressure
// for memory, io and cpu.
#[derive(Debug, Default, Clone)]
pub struct Pressure {
    pub memory: PressureData,
    pub io: PressureData,
    pub cpu: PressureData,
}

// PressureData keeps track of the pressure as reported by kernel psi. For further information
// see https://docs.kernel.org/accounting/psi.html. Content of this property is read directly
// from the kernel {cpu,io,memory}.pressure file.
#[derive(Debug, Default, Clone)]
pub struct PressureData {
    pub some: PressureAverages,
    pub full: PressureAverages,
}

// PressureAverages keeps the averages for 10, 60 and 300 data as present in the kernel psi file.
// The total, also present in the file, is also kept here.
#[derive(Debug, Default, Clone)]
pub struct PressureAverages {
    pub avg10: f32,
    pub avg60: f32,
    pub avg300: f32,
    pub total: f32,
}

// CollectedData holds the collected data for a process. Here, other than the pressure information
// we also keep track of the process memory usage (%) and the oom_score.
#[derive(Debug, Default, Clone)]
pub struct CollectedData {
    pub memory_max: f32,
    pub memory_current: f32,
    pub oom_score: i32,
    pub pressure: Pressure,
}

impl CollectedData {
    // memory_usage returns the memory in use as percentage.
    pub fn memory_usage(&self) -> f32 {
        if self.memory_max > 0. {
            self.memory_current / self.memory_max * 100.
        } else {
            0.
        }
    }
}

// Process holds information about a specific process running on the system.
#[derive(Debug, Default, Clone)]
pub struct Process {
    pub pid: i32,
    pub cmdline: String,
}

// ProcFsReader reads process information from the /proc filesystem using the provided CGroupProvider
// for cgroup-related data.
#[derive(Clone)]
pub struct ProcFsReader<'a> {
    cgroups: &'a dyn cgroups::CGroupProvider,
}

impl<'a> ProcFsReader<'a> {
    // new returns a new ProcFsReader using the provided CGroupProvider.
    pub fn new(cgroups: &'a impl cgroups::CGroupProvider) -> Self {
        ProcFsReader { cgroups }
    }

    // pressure reads all the pressure counters for a given pid.
    fn pressure(&self, pid: i32) -> Result<Pressure, Error> {
        Ok(Pressure {
            memory: self.memory_pressure(pid)?,
            io: self.io_pressure(pid)?,
            cpu: self.cpu_pressure(pid)?,
        })
    }

    // cpu_pressure reads and parses the cpu pressure (psi) for the provided pid.
    fn cpu_pressure(&self, pid: i32) -> Result<PressureData, Error> {
        let path = self.cgroups.path_for_cpu_pressure(pid)?;
        self.parse_pressure_data_file(path)
    }

    // io_pressure reads and parses the io pressure (psi) for the provided pid.
    fn io_pressure(&self, pid: i32) -> Result<PressureData, Error> {
        let path = self.cgroups.path_for_io_pressure(pid)?;
        self.parse_pressure_data_file(path)
    }

    // memory_pressure reads and parses the memory pressure (psi) for the provided pid.
    fn memory_pressure(&self, pid: i32) -> Result<PressureData, Error> {
        let path = self.cgroups.path_for_memory_pressure(pid)?;
        self.parse_pressure_data_file(path)
    }

    // parse_pressure_data_file parses a kernel psi file, the file format is as follow:
    //
    // some avg10=0.00 avg60=0.00 avg300=0.00 total=0
    // full avg10=0.00 avg60=0.00 avg300=0.00 total=0
    fn parse_pressure_data_file(&self, path: String) -> Result<PressureData, Error> {
        let mut result = PressureData::default();
        let fp = fs::File::open(path)?;

        for line in io::BufReader::new(fp).lines() {
            let line = line?;
            let mut tokens = line.trim().split_whitespace();

            let averages = match tokens.next() {
                Some("some") => &mut result.some,
                Some("full") => &mut result.full,
                None => continue,
                Some(other) => return Err(Error::Message(format!("unknown field {}", other))),
            };

            self.parse_pressure_averages(tokens, averages)?;
        }

        Ok(result)
    }

    // parse_pressure_averages parses a list of tokens similar to the following:
    // avg10=0.00 avg60=0.00 avg300=0.00 total=0
    // The provided iterator should iterate over the string split by the white space. Data is
    // parsed and then populated in the provided mutable PressureAverages.
    fn parse_pressure_averages(
        &self,
        tokens: str::SplitWhitespace<'a>,
        averages: &mut PressureAverages,
    ) -> Result<(), Error> {
        for token in tokens {
            let parts: Vec<&str> = token.split("=").collect();
            if parts.len() != 2 {
                continue;
            }
            match parts[0] {
                "avg10" => averages.avg10 = parts[1].parse()?,
                "avg60" => averages.avg60 = parts[1].parse()?,
                "avg300" => averages.avg300 = parts[1].parse()?,
                "total" => averages.total = parts[1].parse()?,
                _ => return Err(Error::Message(format!("unknown field {}", parts[0]))),
            }
        }
        Ok(())
    }

    // memory_stats returns stats about memory utilization for the provided process id. Values are
    // returned as a tuple where the first element is the current memory utilization and the second
    // the maximum allowed. This data is read from the cgroup's /proc files.
    fn memory_stats(&self, pid: i32) -> Result<(f32, f32), Error> {
        let path = self.cgroups.path_to_memory_max(pid)?;
        let memory_max = fs::read_to_string(path)?;
        let memory_max: i32 = memory_max.trim().parse()?;

        let path = self.cgroups.path_to_memory_current(pid)?;
        let memory_current = fs::read_to_string(path)?;
        let memory_current: i32 = memory_current.trim().parse()?;
        Ok((memory_current as f32, memory_max as f32))
    }

    // has_memory_limit returns true if the provided pid has an upper limit on how much memory it
    // can use. Kernel sets the limit to the string 'max' if no upper limit is set.
    fn has_memory_limit(&self, pid: i32) -> Result<bool, Error> {
        let path = self.cgroups.path_to_memory_max(pid)?;
        let memory_max = fs::read_to_string(path)?;
        match memory_max.trim().parse::<i32>() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // oom_score returns the oom score for a given pid. The score is calculated by reading the
    // oom_score and then adding the oom_score_adj. Before applying oom_score_adj an oom score
    // is on the 0-1000 range (the higher the most likely for the process to be chosen during
    // an OOMKill event). XXX processes owned by "root" have an automatic adjustment of -30 but
    // we are not taking that into account.
    fn oom_score(&self, pid: i32) -> Result<i32, Error> {
        let path = format!("/proc/{}/oom_score", pid);
        let oom_score = fs::read_to_string(path)?;
        let oom_score: i32 = oom_score.trim().parse()?;

        let path = format!("/proc/{}/oom_score_adj", pid);
        let oom_score_adj = fs::read_to_string(path)?;
        let oom_score_adj: i32 = oom_score_adj.trim().parse()?;

        Ok(oom_score + oom_score_adj)
    }

    // cmdline reads the cmdline for a given pid. Commands on cmdline are defined as a string
    // where the command and the arguments are separated by a \0. This function returns only
    // the first part (ignores the arguments).
    fn cmdline(&self, pid: i32) -> Result<String, Error> {
        let path = format!("/proc/{}/cmdline", pid);
        let cmdline = fs::read_to_string(path)?;
        let slices = cmdline.split('\0');
        let slices: Vec<&str> = slices.collect();
        Ok(slices[0].to_string())
    }
}

// ProcessProvider is a trait implemented by any entity capable of providing process information.
pub trait ProcessProvider {
    fn list(&self) -> Result<Vec<Process>, Error>;
    fn collect_process_data(&self, pid: i32) -> Result<CollectedData, Error>;
    fn send_signal(&self, pid: i32, sig: signal::Signal) -> Result<(), Error>;
}

impl<'a> ProcessProvider for ProcFsReader<'a> {
    // list processes entries under the /proc filesystem and returns a list of Process. Due to the
    // nature of /proc filesystem there are no guarantees that the returned list is the complete set,
    // processes come and go as they please. A failure to read a path in /proc is considered a normal
    // occurrence and is just skipped.
    fn list(&self) -> Result<Vec<Process>, Error> {
        let dir_entries = fs::read_dir("/proc")?;

        let mut processes: Vec<Process> = vec![];
        for tmp_entry in dir_entries {
            let entry = tmp_entry?;

            // processes come and go as they please so here if we can't get the file metadata
            // then it most likely went way. We just move on.
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(err) => {
                    debug!("reading entry metadata: {err}");
                    continue;
                }
            };

            if metadata.is_dir() == false {
                continue;
            }

            let pbuf = entry.path();
            let Some(path) = pbuf.as_path().file_name() else {
                continue;
            };

            let Some(as_str) = path.to_str() else {
                continue;
            };

            let Ok(pid): Result<i32, _> = as_str.parse() else {
                continue;
            };

            processes.push(Process {
                pid,
                cmdline: self.cmdline(pid).unwrap_or_default(),
            });
        }

        Ok(processes)
    }

    // collect_process_data reads all data for a given process identified by the pid. Returns a
    // collected data struct or an error. XXX pressure reads are skipped from cgroups v1. If
    // the process has no memory limit then its memory usage is 0%.
    fn collect_process_data(&self, pid: i32) -> Result<CollectedData, Error> {
        let mut result = CollectedData::default();
        result.oom_score = self.oom_score(pid)?;

        if self.has_memory_limit(pid)? {
            (result.memory_current, result.memory_max) = self.memory_stats(pid)?;
        }

        Ok(match self.cgroups.version()? {
            cgroups::CGroupsVersions::CGroupsV1 => result,
            cgroups::CGroupsVersions::CGroupsV2 => {
                result.pressure = self.pressure(pid)?;
                result
            }
        })
    }

    // send_signal sends a signal to the process pointed by the pid.
    fn send_signal(&self, pid: i32, sig: signal::Signal) -> Result<(), Error> {
        let pid = unistd::Pid::from_raw(pid);
        signal::kill(pid, sig)?;
        Ok(())
    }
}
