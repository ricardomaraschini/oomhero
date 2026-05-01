use super::errors::Error;
use nix::sys::statfs;

// CGroupsVersions is an enum of all supported versions of the kernel cgroups feature.
#[derive(Debug, Clone, Default)]
pub enum CGroupsVersions {
    #[default]
    CGroupsV1,
    CGroupsV2,
}

// CGroupProvider is a trait implemented by any entity capable of providing information about
// cgroups on the system.
pub trait CGroupProvider {
    fn version(&self) -> Result<CGroupsVersions, Error>;
    fn path_to_memory_max(&self, pid: i32) -> Result<String, Error>;
    fn path_to_memory_current(&self, pid: i32) -> Result<String, Error>;
    fn path_for_memory_pressure(&self, pid: i32) -> Result<String, Error>;
    fn path_for_io_pressure(&self, pid: i32) -> Result<String, Error>;
    fn path_for_cpu_pressure(&self, pid: i32) -> Result<String, Error>;
}

// SystemCGroups provides cgroup information by reading from the system's /sys/fs/cgroup filesystem.
#[derive(Debug, Clone, Default)]
pub struct SystemCGroups {}

impl SystemCGroups {
    // path_to_pressure_file returns the path to the pressure file for the provided resource.
    // Pressure isn't supported on CGroupsV1.
    fn path_to_pressure_file(&self, pid: i32, resource: &str) -> Result<String, Error> {
        match self.version()? {
            CGroupsVersions::CGroupsV1 => {
                Err(Error::Message("pressure not supported on v1".to_string()))
            }
            CGroupsVersions::CGroupsV2 => Ok(format!(
                "/proc/{}/root/sys/fs/cgroup/{}.pressure",
                pid, resource
            )),
        }
    }
}

impl CGroupProvider for SystemCGroups {
    // version returns the cgroups version supported by the kernel. This is determined by inspecting
    // the /sys/fs filesystem.
    fn version(&self) -> Result<CGroupsVersions, Error> {
        Ok(match statfs::statfs("/sys/fs/cgroup")?.filesystem_type() {
            statfs::CGROUP2_SUPER_MAGIC => CGroupsVersions::CGroupsV2,
            _ => CGroupsVersions::CGroupsV1,
        })
    }

    // path_to_memory_max returns the path to the file from where the max memory allowance for a
    // given pid can be read from. The path varies according to the supported cgroups version.
    fn path_to_memory_max(&self, pid: i32) -> Result<String, Error> {
        Ok(match self.version()? {
            CGroupsVersions::CGroupsV1 => {
                format!(
                    "/proc/{}/root/sys/fs/cgroup/memory/memory.limit_in_bytes",
                    pid
                )
            }
            CGroupsVersions::CGroupsV2 => {
                format!("/proc/{}/root/sys/fs/cgroup/memory.max", pid)
            }
        })
    }

    // path_to_memory_current returns the path to the file from where the current memory usage for
    // a given pid can be read from. The path varies according to the supported cgroups version.
    fn path_to_memory_current(&self, pid: i32) -> Result<String, Error> {
        Ok(match self.version()? {
            CGroupsVersions::CGroupsV1 => {
                format!(
                    "/proc/{}/root/sys/fs/cgroup/memory/memory.usage_in_bytes",
                    pid
                )
            }
            CGroupsVersions::CGroupsV2 => {
                format!("/proc/{}/root/sys/fs/cgroup/memory.current", pid)
            }
        })
    }

    // path_for_memory_pressure returns the path from where to read the memory pressure information.
    // This is not supported on cgroups v1 (it might be but it is not coded here).
    fn path_for_memory_pressure(&self, pid: i32) -> Result<String, Error> {
        self.path_to_pressure_file(pid, "memory")
    }

    // path_for_io_pressure returns the path from where to read the io pressure information. This is
    // not supported on cgroups v1 (it might be but it is not coded here).
    fn path_for_io_pressure(&self, pid: i32) -> Result<String, Error> {
        self.path_to_pressure_file(pid, "io")
    }

    // path_for_cpu_pressure returns the path from where to read the cpu pressure information. This is
    // not supported on cgroups v1 (it might be but it is not coded here).
    fn path_for_cpu_pressure(&self, pid: i32) -> Result<String, Error> {
        self.path_to_pressure_file(pid, "cpu")
    }
}
