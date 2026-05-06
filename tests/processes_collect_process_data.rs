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

    let expected = processes::Pressure {
        memory: processes::PressureData {
            some: processes::PressureAverages {
                avg10: 1.,
                avg60: 2.,
                avg300: 3.,
                total: 4.,
            },
            full: processes::PressureAverages {
                avg10: 5.,
                avg60: 6.,
                avg300: 7.,
                total: 8.,
            },
        },
        io: processes::PressureData {
            some: processes::PressureAverages {
                avg10: 9.,
                avg60: 10.,
                avg300: 11.,
                total: 12.,
            },
            full: processes::PressureAverages {
                avg10: 13.,
                avg60: 14.,
                avg300: 15.,
                total: 16.,
            },
        },
        cpu: processes::PressureData {
            some: processes::PressureAverages {
                avg10: 17.,
                avg60: 18.,
                avg300: 19.,
                total: 20.,
            },
            full: processes::PressureAverages {
                avg10: 21.,
                avg60: 22.,
                avg300: 23.,
                total: 24.,
            },
        },
    };

    assert_eq!(result.pressure, expected);
    Ok(())
}
