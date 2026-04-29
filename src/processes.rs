use super::errors::Error;
use nix::sys::signal;
use nix::unistd;
use std::fs;
use std::io;
use std::io::BufRead;
use std::str;

// PressureData keeps track of the presssure as reported by kernel psi. For further information
// see https://docs.kernel.org/accounting/psi.html.
#[derive(Debug, Default)]
pub struct PressureData {
    pub some: PressureAverages,
    pub full: PressureAverages,
}

// PressureAverages keeps the averages for 10, 60 and 300 data as present in the kernel psi file.
// the total, also present in the file, is also kept here.
#[derive(Debug, Default)]
pub struct PressureAverages {
    pub avg10: f32,
    pub avg60: f32,
    pub avg300: f32,
    pub total: f32,
}

// Pressure gather all the pressure data we read for each single process.
#[derive(Debug, Default)]
pub struct Pressure {
    pub memory: PressureData,
    pub cpu: PressureData,
    pub io: PressureData,
}

// Process holds information about a specific process running on the system.
#[derive(Debug, Default)]
pub struct Process {
    pub pid: i32,
    pub cmdline: String,
    pub pressure: Pressure,
}

// list processes entries under the /proc filesystem and returns a list of pids. Due to the nature
// of /proc filesystem there is no guarantee that the returned list is the complete set, processes
// come and go as they please.
pub fn list() -> Result<Vec<Process>, Error> {
    let dir_entries = fs::read_dir("/proc")?;

    let mut processes: Vec<Process> = vec![];
    for tmp_entry in dir_entries {
        let entry = tmp_entry?;

        // processes come and go as they please so here if we can't get the file metadata
        // then it most likely went way. We just move on.
        let Ok(metadata) = entry.metadata() else {
            continue;
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
            cmdline: cmdline(pid).unwrap_or_default(),
            pressure: Pressure {
                memory: memory_pressure(pid).unwrap_or_default(),
                cpu: cpu_pressure(pid).unwrap_or_default(),
                io: io_pressure(pid).unwrap_or_default(),
            },
        });
    }

    Ok(processes)
}

// cpu_pressure reads and parses the cpu pressure (psi) for the provided pid.
pub fn cpu_pressure(pid: i32) -> Result<PressureData, Error> {
    let path = super::cgroups::path_for_cpu_pressure(pid)?;
    return parse_pressure_data_file(path);
}

// io_pressure reads and parses the io pressure (psi) for the provided pid.
pub fn io_pressure(pid: i32) -> Result<PressureData, Error> {
    let path = super::cgroups::path_for_io_pressure(pid)?;
    return parse_pressure_data_file(path);
}

// memory_pressure reads and parses the memory pressure (psi) for the provided pid.
pub fn memory_pressure(pid: i32) -> Result<PressureData, Error> {
    let path = super::cgroups::path_for_memory_pressure(pid)?;
    return parse_pressure_data_file(path);
}

// parse_pressure_data_file parses a kernel psi file, the file format is as follow:
//
// some avg10=0.00 avg60=0.00 avg300=0.00 total=0
// full avg10=0.00 avg60=0.00 avg300=0.00 total=0
fn parse_pressure_data_file(path: String) -> Result<PressureData, Error> {
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

        parse_pressure_averages(tokens, averages)?;
    }

    Ok(result)
}

// parse_pressure_averages parses a list of tokens similar to the following:
// avg10=0.00 avg60=0.00 avg300=0.00 total=0
// The provided iterator should iterate over the string splitted by the white space. Data is
// parsed and then populated in the provided mutable PressureAverages.
fn parse_pressure_averages<'a>(
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

// memory_stats returns stats about memory utilization for the provided process id. values are
// returned as a tuple where the first element is the current memory utilization and the second
// the maximum allowed. This data is read from the cgroup's /proc files.
pub fn memory_stats(pid: i32) -> Result<(i32, i32), Error> {
    let path = super::cgroups::path_to_memory_max(pid)?;
    let memory_max = fs::read_to_string(path)?;
    let memory_max: i32 = memory_max.trim().parse()?;

    let path = super::cgroups::path_to_memory_current(pid)?;
    let memory_current = fs::read_to_string(path)?;
    let memory_current: i32 = memory_current.trim().parse()?;
    Ok((memory_current, memory_max))
}

// has_memory_limit returns true if the provided pid has an upper limit on how much memory it
// can use. kernel set the limit to the string 'max' if no uper limit is set.
pub fn has_memory_limit(pid: i32) -> Result<bool, Error> {
    let path = super::cgroups::path_to_memory_max(pid)?;
    let memory_max = fs::read_to_string(path)?;
    match memory_max.trim().parse::<i32>() {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

// oom_score returns the oom score for a given pid. The score is calculated by reading the
// oom_score and then adding the oom_score_adj. Before applying oom_score_adj an oom score
// is on the 0-1000 range (the higher the most likely for the process to be chosend during
// an OOMKill event). XXX processes owned by "root" have an automatic adjustment of -30 but
// we are not taking that into account.
pub fn oom_score(pid: i32) -> Result<i32, Error> {
    let path = format!("/proc/{}/oom_score", pid);
    let oom_score = fs::read_to_string(path)?;
    let oom_score: i32 = oom_score.trim().parse()?;

    let path = format!("/proc/{}/oom_score_adj", pid);
    let oom_score_adj = fs::read_to_string(path)?;
    let oom_score_adj: i32 = oom_score_adj.trim().parse()?;

    Ok(oom_score + oom_score_adj)
}

// cmdline reads the cmdline for a given pid. Commands on cmdline is defined as a string
// where the command and the arguments are separated by a \0. This function returns only
// the first part (ignore the arguments).
pub fn cmdline(pid: i32) -> Result<String, Error> {
    let path = format!("/proc/{}/cmdline", pid);
    let cmdline = fs::read_to_string(path)?;
    let slices = cmdline.split('\0');
    let slices: Vec<&str> = slices.collect();
    Ok(slices[0].to_string())
}

// send_signal sends a signal to the process pointed by the pid.
pub fn send_signal(pid: i32, sig: signal::Signal) -> Result<(), Error> {
    let pid = unistd::Pid::from_raw(pid);
    signal::kill(pid, sig)?;
    Ok(())
}
