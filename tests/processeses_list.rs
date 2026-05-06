use oomhero::errors;
use oomhero::processes;
use oomhero::processes::ProcessProvider;
use oomhero::system;
use std::env;

fn fake_system_profs() -> Result<impl system::Provider, errors::Error> {
    let mut root = env::current_dir()?;
    root.push("tests/data/processes_list");
    let root = root.to_string_lossy().into_owned();
    Ok(system::SystemCGroups::default().with_procfs_root(root))
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
