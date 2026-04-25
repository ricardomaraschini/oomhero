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
