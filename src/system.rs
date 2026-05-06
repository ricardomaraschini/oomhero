use super::errors::Error;
use mockall::automock;
use nix::sys::statfs;
use std::path;

// CGroupsVersions is an enum of all supported versions of the kernel cgroups feature.
#[derive(Debug, Clone, Default)]
pub enum CGroupsVersions {
    #[default]
    CGroupsV1,
    CGroupsV2,
}

// Provider is a trait implemented by any entity capable of providing information about the
// operating system. It provide tooling to find the right paths for crucial files we need
// access to.
#[automock]
pub trait Provider {
    fn cgroups_version(&self) -> Result<CGroupsVersions, Error>;
    fn path_to_memory_max(&self, pid: i32) -> Result<path::PathBuf, Error>;
    fn path_to_memory_current(&self, pid: i32) -> Result<path::PathBuf, Error>;
    fn path_to_memory_pressure(&self, pid: i32) -> Result<path::PathBuf, Error>;
    fn path_to_io_pressure(&self, pid: i32) -> Result<path::PathBuf, Error>;
    fn path_to_cpu_pressure(&self, pid: i32) -> Result<path::PathBuf, Error>;
    fn path_to_oom_score(&self, pid: i32) -> path::PathBuf;
    fn path_to_oom_score_adj(&self, pid: i32) -> path::PathBuf;
    fn path_to_cmdline(&self, pid: i32) -> path::PathBuf;
    fn path_to_procfs(&self) -> path::PathBuf;
}

// SystemCGroups provides cgroup information by reading from the system's /sys/fs/cgroup filesystem.
#[derive(Debug, Clone)]
pub struct SystemCGroups {
    procfs_root: String,
}

impl SystemCGroups {
    // default returns a default SystemCGroups with the procfs root set to /proc.
    pub fn default() -> Self {
        SystemCGroups {
            procfs_root: String::from("/proc"),
        }
    }

    // with_procfs_root allows for customization of the procfs root path.
    pub fn with_procfs_root(mut self, root: String) -> Self {
        self.procfs_root = root;
        self
    }

    // path_to_pressure_file returns the path to the pressure file for the provided resource.
    // Pressure isn't supported on CGroupsV1.
    fn path_to_pressure_file(&self, pid: i32, resource: &str) -> Result<path::PathBuf, Error> {
        self.cgroups_version().and_then(|version| match version {
            CGroupsVersions::CGroupsV1 => {
                Err(Error::Message("pressure not supported on v1".to_string()))
            }
            CGroupsVersions::CGroupsV2 => Ok(path::PathBuf::from(format!(
                "{}/{}/root/sys/fs/cgroup/{}.pressure",
                self.procfs_root, pid, resource
            ))),
        })
    }
}

impl Provider for SystemCGroups {
    // cgroups_version returns the cgroups version supported by the kernel. This is determined by
    // inspecting the /sys/fs filesystem.
    fn cgroups_version(&self) -> Result<CGroupsVersions, Error> {
        Ok(match statfs::statfs("/sys/fs/cgroup")?.filesystem_type() {
            statfs::CGROUP2_SUPER_MAGIC => CGroupsVersions::CGroupsV2,
            _ => CGroupsVersions::CGroupsV1,
        })
    }

    // path_to_memory_max returns the path to the file from where the max memory allowance for a
    // given pid can be read from. The path varies according to the supported cgroups version.
    fn path_to_memory_max(&self, pid: i32) -> Result<path::PathBuf, Error> {
        self.cgroups_version().map(|version| match version {
            CGroupsVersions::CGroupsV1 => path::PathBuf::from(format!(
                "{}/{}/root/sys/fs/cgroup/memory/memory.limit_in_bytes",
                self.procfs_root, pid
            )),
            CGroupsVersions::CGroupsV2 => path::PathBuf::from(format!(
                "{}/{}/root/sys/fs/cgroup/memory.max",
                self.procfs_root, pid
            )),
        })
    }

    // path_to_memory_current returns the path to the file from where the current memory usage for
    // a given pid can be read from. The path varies according to the supported cgroups version.
    fn path_to_memory_current(&self, pid: i32) -> Result<path::PathBuf, Error> {
        self.cgroups_version().map(|version| match version {
            CGroupsVersions::CGroupsV1 => path::PathBuf::from(format!(
                "{}/{}/root/sys/fs/cgroup/memory/memory.usage_in_bytes",
                self.procfs_root, pid
            )),
            CGroupsVersions::CGroupsV2 => path::PathBuf::from(format!(
                "{}/{}/root/sys/fs/cgroup/memory.current",
                self.procfs_root, pid
            )),
        })
    }

    // path_to_memory_pressure returns the path from where to read the memory pressure information.
    // This is not supported on cgroups v1 (it might be but it is not coded here).
    fn path_to_memory_pressure(&self, pid: i32) -> Result<path::PathBuf, Error> {
        self.path_to_pressure_file(pid, "memory")
    }

    // path_to_io_pressure returns the path from where to read the io pressure information. This is
    // not supported on cgroups v1 (it might be but it is not coded here).
    fn path_to_io_pressure(&self, pid: i32) -> Result<path::PathBuf, Error> {
        self.path_to_pressure_file(pid, "io")
    }

    // path_to_cpu_pressure returns the path from where to read the cpu pressure information. This is
    // not supported on cgroups v1 (it might be but it is not coded here).
    fn path_to_cpu_pressure(&self, pid: i32) -> Result<path::PathBuf, Error> {
        self.path_to_pressure_file(pid, "cpu")
    }

    // path_to_oom_score returns the path from where to read a process oom score.
    fn path_to_oom_score(&self, pid: i32) -> path::PathBuf {
        path::PathBuf::from(format!("{}/{}/oom_score", self.procfs_root, pid))
    }

    // path_to_oom_score_adj returns the path from where to read a process oom score adjustment.
    fn path_to_oom_score_adj(&self, pid: i32) -> path::PathBuf {
        path::PathBuf::from(format!("{}/{}/oom_score_adj", self.procfs_root, pid))
    }

    // path_to_cmdline returns the path from where to read a process command line.
    fn path_to_cmdline(&self, pid: i32) -> path::PathBuf {
        path::PathBuf::from(format!("{}/{}/cmdline", self.procfs_root, pid))
    }

    // path_to_procfs returns the path from where read the procfs. This exists mostly so we can
    // mock this whole struct as this isn't expected to differ in any way.
    fn path_to_procfs(&self) -> path::PathBuf {
        path::PathBuf::from(format!("{}", self.procfs_root))
    }
}
