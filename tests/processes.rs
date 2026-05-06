use oomhero::errors;
use oomhero::processes;
use oomhero::processes::ProcessProvider;
use oomhero::system;
use std::env;
use std::path;

fn fake_system_profs() -> Result<impl system::Provider, errors::Error> {
    let mut root = env::current_dir()?;
    root.push("tests/data");
    let root = root.to_string_lossy().into_owned();
    Ok(system::SystemCGroups::default().with_procfs_root(root))
}

fn path_buf_to_pid(pid: i32) -> String {
    String::from(format!("tests/data/{}", pid))
}

#[test]
fn list_works() -> Result<(), errors::Error> {
    let sysfs = fake_system_profs()?;
    let proc = processes::ProcFsReader::new(sysfs);
    let result = proc.list()?;
    assert_eq!(result.len(), 5);
    Ok(())
}

#[test]
fn list_fails() -> Result<(), errors::Error> {
    let sysfs = system::SystemCGroups::default().with_procfs_root("does-not-exist".to_string());
    let proc = processes::ProcFsReader::new(sysfs);
    let result = proc.list();
    assert!(matches!(result, Err(errors::Error::Io(_))));
    Ok(())
}

#[test]
fn collect_process_data() -> Result<(), errors::Error> {
    let mut mock = system::MockProvider::new();
    mock.expect_path_to_oom_score()
        .returning(|pid| path::PathBuf::from(format!("{}/oom_score", path_buf_to_pid(pid))));
    mock.expect_path_to_oom_score_adj()
        .returning(|pid| path::PathBuf::from(format!("{}/oom_score_adj", path_buf_to_pid(pid))));
    mock.expect_path_to_memory_max().returning(|pid| {
        Ok(path::PathBuf::from(format!(
            "{}/memory_max",
            path_buf_to_pid(pid)
        )))
    });
    mock.expect_path_to_memory_current().returning(|pid| {
        Ok(path::PathBuf::from(format!(
            "{}/memory_current",
            path_buf_to_pid(pid)
        )))
    });
    mock.expect_cgroups_version()
        .returning(|| Ok(system::CGroupsVersions::CGroupsV2));
    mock.expect_path_to_memory_pressure().returning(|pid| {
        Ok(path::PathBuf::from(format!(
            "{}/memory_pressure",
            path_buf_to_pid(pid)
        )))
    });

    let proc = processes::ProcFsReader::new(mock);
    println!("{:?}", proc.collect_process_data(1));
    Ok(())
}
