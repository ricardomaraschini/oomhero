use super::errors::Error;
use nix::sys::statfs;

// CGroupsVersions is an enum of all supported versions of the kernel cgroups feature.
#[derive(Debug)]
pub enum CGroupsVersions {
    CGroupsV1,
    CGroupsV2,
}

// version returns the cgroups version supported by the kernel.This is determined by inspecting
// the /sys/fs filesystem.
pub fn version() -> Result<CGroupsVersions, Error> {
    let stat = statfs::statfs("/sys/fs/cgroup")?;
    if stat.filesystem_type() == statfs::CGROUP2_SUPER_MAGIC {
        return Ok(CGroupsVersions::CGroupsV2);
    }
    Ok(CGroupsVersions::CGroupsV1)
}

// path_to_memory_max returns the path to the file from where the max memory allowance for a
// given pid can be read from. the path varies according to the supported cgroups version.
pub fn path_to_memory_max(pid: i32) -> Result<String, Error> {
    let version = version()?;
    let path = match version {
        CGroupsVersions::CGroupsV1 => {
            format!(
                "/proc/{}/root/sys/fs/cgroup/memory/memory.limit_in_bytes",
                pid
            )
        }
        CGroupsVersions::CGroupsV2 => {
            format!("/proc/{}/root/sys/fs/cgroup/memory.max", pid)
        }
    };
    Ok(path)
}

// path_to_memory_current returns the path to the file from where the current memory usage for
// a given pid can be read from. the path varies according to the supported cgroups version.
pub fn path_to_memory_current(pid: i32) -> Result<String, Error> {
    let version = version()?;
    let path = match version {
        CGroupsVersions::CGroupsV1 => {
            format!(
                "/proc/{}/root/sys/fs/cgroup/memory/memory.usage_in_bytes",
                pid
            )
        }
        CGroupsVersions::CGroupsV2 => {
            format!("/proc/{}/root/sys/fs/cgroup/memory.current", pid)
        }
    };
    Ok(path)
}

// path_for_memory_pressure returns the path from where to read the memory pressure information.
// this is not supported on cgroups v1 (it might be but it is not coded here).
pub fn path_for_memory_pressure(pid: i32) -> Result<String, Error> {
    let version = version()?;
    if let CGroupsVersions::CGroupsV1 = version {
        return Err(Error::Message(format!("pressure not supported on v1")));
    };
    Ok(format!("/proc/{}/root/sys/fs/cgroup/memory.pressure", pid))
}

// path_for_io_pressure returns the path from where to read the io pressure information. this is
// not supported on cgroups v1 (it might be but it is not coded here).
pub fn path_for_io_pressure(pid: i32) -> Result<String, Error> {
    let version = version()?;
    if let CGroupsVersions::CGroupsV1 = version {
        return Err(Error::Message(format!("pressure not supported on v1")));
    };
    Ok(format!("/proc/{}/root/sys/fs/cgroup/io.pressure", pid))
}

// path_for_cpu_pressure returns the path from where to read the io pressure information. this is
// not supported on cgroups v1 (it might be but it is not coded here).
pub fn path_for_cpu_pressure(pid: i32) -> Result<String, Error> {
    let version = version()?;
    if let CGroupsVersions::CGroupsV1 = version {
        return Err(Error::Message(format!("pressure not supported on v1")));
    };
    Ok(format!("/proc/{}/root/sys/fs/cgroup/cpu.pressure", pid))
}
