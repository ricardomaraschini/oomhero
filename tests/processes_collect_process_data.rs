use oomhero::errors;
use oomhero::processes;
use oomhero::processes::ProcessProvider;
use oomhero::system;
use std::path;

fn path_buf_to_fake_process(name: &String) -> String {
    String::from(format!(
        "tests/data/processes_collect_process_data/{}",
        name
    ))
}

fn mock_for_fake_process(name: &String) -> system::MockProvider {
    let mut mock = system::MockProvider::new();

    mock.expect_cgroups_version()
        .returning(|| Ok(system::CGroupsVersions::CGroupsV2));

    let oom_score_path = format!("{}/oom_score", path_buf_to_fake_process(name));
    mock.expect_path_to_oom_score()
        .returning(move |_pid| path::PathBuf::from(oom_score_path.clone()));

    let oom_score_adj_path = format!("{}/oom_score_adj", path_buf_to_fake_process(name));
    mock.expect_path_to_oom_score_adj()
        .returning(move |_pid| path::PathBuf::from(oom_score_adj_path.clone()));

    let memory_max_path = format!("{}/memory_max", path_buf_to_fake_process(name));
    mock.expect_path_to_memory_max()
        .returning(move |_pid| Ok(path::PathBuf::from(memory_max_path.clone())));

    let memory_current_path = format!("{}/memory_current", path_buf_to_fake_process(name));
    mock.expect_path_to_memory_current()
        .returning(move |_pid| Ok(path::PathBuf::from(memory_current_path.clone())));

    let memory_pressure_path = format!("{}/memory_pressure", path_buf_to_fake_process(name));
    mock.expect_path_to_memory_pressure()
        .returning(move |_pid| Ok(path::PathBuf::from(memory_pressure_path.clone())));

    let io_pressure_path = format!("{}/io_pressure", path_buf_to_fake_process(name));
    mock.expect_path_to_io_pressure()
        .returning(move |_pid| Ok(path::PathBuf::from(io_pressure_path.clone())));

    let cpu_pressure_path = format!("{}/cpu_pressure", path_buf_to_fake_process(name));
    mock.expect_path_to_cpu_pressure()
        .returning(move |_pid| Ok(path::PathBuf::from(cpu_pressure_path.clone())));

    mock
}

#[test]
fn collect_process_data_no_oom_score() -> Result<(), errors::Error> {
    let proc_name = String::from("no_oom_score");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1);
    assert!(matches!(result, Err(errors::Error::Io(_))));
    Ok(())
}

#[test]
fn collect_process_data_no_oom_score_adj() -> Result<(), errors::Error> {
    let proc_name = String::from("no_oom_score_adj");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1);
    assert!(matches!(result, Err(errors::Error::Io(_))));
    Ok(())
}

#[test]
fn collect_process_data_invalid_oom_score_adj() -> Result<(), errors::Error> {
    let proc_name = String::from("invalid_oom_score_adj");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1);
    assert!(matches!(result, Err(errors::Error::ParseIntError(_))));
    Ok(())
}

#[test]
fn collect_process_data_no_memory_max_file() -> Result<(), errors::Error> {
    let proc_name = String::from("no_memory_max_file");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1);
    assert!(matches!(result, Err(errors::Error::Io(_))));
    Ok(())
}

#[test]
fn collect_process_data_unlimited_memory_process() -> Result<(), errors::Error> {
    let proc_name = String::from("unlimited_memory_process");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1)?;
    assert_eq!(result.oom_score, 0);
    assert_eq!(result.memory_max, 0.);
    assert_eq!(result.memory_current, 0.);
    assert_eq!(result.memory_usage(), 0.);
    Ok(())
}

#[test]
fn collect_process_data_10_pct_memory_usage() -> Result<(), errors::Error> {
    let proc_name = String::from("10_pct_memory_usage");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1)?;
    assert_eq!(result.oom_score, 0);
    assert_eq!(result.memory_max, 1000.);
    assert_eq!(result.memory_current, 100.);
    assert_eq!(result.memory_usage(), 10.);
    Ok(())
}

#[test]
fn collect_process_data_oom_with_adjustment() -> Result<(), errors::Error> {
    let proc_name = String::from("oom_with_adjustment");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1)?;
    assert_eq!(result.oom_score, 900);
    Ok(())
}

#[test]
fn collect_process_data_invalid_pressure_data() -> Result<(), errors::Error> {
    let proc_name = String::from("invalid_pressure_data");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1);
    assert!(matches!(result, Err(errors::Error::ParseFloatError(_))));
    Ok(())
}

#[test]
fn collect_process_data_full_pressure_data() -> Result<(), errors::Error> {
    let proc_name = String::from("full_pressure_data");
    let mock = mock_for_fake_process(&proc_name);
    let proc = processes::ProcFsReader::new(mock);
    let result = proc.collect_process_data(1)?;
    assert_eq!(result.pressure.memory.some.avg10, 1.);
    assert_eq!(result.pressure.memory.some.avg60, 2.);
    assert_eq!(result.pressure.memory.some.avg300, 3.);
    assert_eq!(result.pressure.memory.some.total, 4.);
    assert_eq!(result.pressure.memory.full.avg10, 5.);
    assert_eq!(result.pressure.memory.full.avg60, 6.);
    assert_eq!(result.pressure.memory.full.avg300, 7.);
    assert_eq!(result.pressure.memory.full.total, 8.);

    assert_eq!(result.pressure.io.some.avg10, 9.);
    assert_eq!(result.pressure.io.some.avg60, 10.);
    assert_eq!(result.pressure.io.some.avg300, 11.);
    assert_eq!(result.pressure.io.some.total, 12.);
    assert_eq!(result.pressure.io.full.avg10, 13.);
    assert_eq!(result.pressure.io.full.avg60, 14.);
    assert_eq!(result.pressure.io.full.avg300, 15.);
    assert_eq!(result.pressure.io.full.total, 16.);

    assert_eq!(result.pressure.cpu.some.avg10, 17.);
    assert_eq!(result.pressure.cpu.some.avg60, 18.);
    assert_eq!(result.pressure.cpu.some.avg300, 19.);
    assert_eq!(result.pressure.cpu.some.total, 20.);
    assert_eq!(result.pressure.cpu.full.avg10, 21.);
    assert_eq!(result.pressure.cpu.full.avg60, 22.);
    assert_eq!(result.pressure.cpu.full.avg300, 23.);
    assert_eq!(result.pressure.cpu.full.total, 24.);

    Ok(())
}
