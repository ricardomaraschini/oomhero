use futures_util::StreamExt;
use log::info;
use oomhero::http_signals_sender;
use podman_api::models::LinuxCpu;
use podman_api::models::LinuxMemory;
use podman_api::models::LinuxResources;
use podman_api::models::NamedVolume;
use podman_api::models::PortMapping;
use podman_api::opts;
use podman_api::Podman;
use serde::Deserialize;
use std::env;
use std::fs;
use std::time::Duration;
use uzers::get_current_uid;

// WORKLOAD_IMAGE is the image that simulates an actual workload on a cluster. It is the
// application that is monitored by the oomhero container, receives signals and  reacts
// to them. During e2e the image under tests/image is used. For tests to work this image
// is expected to be already present in the podman storage.
const WORKLOAD_IMAGE: &str = "test-workload";

// OOMHERO_IMAGE is the oomhero version we are testing. This image is expected to be present
// in the podman storage prior to run the tests. Before running the test make sure you
// built the image.
const OOMHERO_IMAGE: &str = "ghcr.io/ricardomaraschini/oomhero";

// Stats represents the response from the /stats endpoint of the workload container.
#[derive(Deserialize, Debug)]
struct Stats {
    signals_received: i32,
}

// workload_container_resource_limits  returns the limits to be used in the workload
// container. We limit the amount of resources that the test workload container can
// use to make testing possible.
async fn workload_container_resource_limits() -> LinuxResources {
    LinuxResources {
        cpu: Some(LinuxCpu {
            period: Some(1_000_000),
            quota: Some(100_000),
            cpus: None,
            mems: None,
            realtime_period: None,
            realtime_runtime: None,
            shares: None,
        }),
        memory: Some(LinuxMemory {
            limit: Some(67_108_864),
            disable_oom_killer: None,
            kernel: None,
            kernel_tcp: None,
            reservation: None,
            swap: None,
            swappiness: None,
            use_hierarchy: None,
        }),
        block_io: None,
        devices: None,
        hugepage_limits: None,
        network: None,
        pids: None,
        rdma: None,
        unified: None,
    }
}

// oomhero_container_resource_limits returns the container limits to be applied to the oomhero
// container during tests.
fn oomhero_container_resource_limits() -> LinuxResources {
    LinuxResources {
        cpu: Some(LinuxCpu {
            period: Some(1_000_000),
            quota: Some(100_000),
            cpus: None,
            mems: None,
            realtime_period: None,
            realtime_runtime: None,
            shares: None,
        }),
        memory: Some(LinuxMemory {
            limit: Some(33_554_432),
            disable_oom_killer: None,
            kernel: None,
            kernel_tcp: None,
            reservation: None,
            swap: None,
            swappiness: None,
            use_hierarchy: None,
        }),
        block_io: None,
        devices: None,
        hugepage_limits: None,
        network: None,
        pids: None,
        rdma: None,
        unified: None,
    }
}

// podman_client returns a client pointing to the podman socket. The socket is expected to be under
// $XDG_RUNTIME_DIR/podman/podman.sock for regular users while for root we use the socket under
// /run/podman/podman.sock.
fn podman_client() -> Podman {
    if get_current_uid() == 0 {
        return Podman::unix("/run/podman/podman.sock");
    }
    let runtime_dir = env::var("XDG_RUNTIME_DIR").expect("failed to read xdg runtime dir");
    let socket_path = format!("{}/podman/podman.sock", runtime_dir);
    Podman::unix(socket_path)
}

// create_test_pod will create a pod with three containers, one with the pause image, one with the
// test image (see tests/image directory) and one with the oomhero. The arguments to the oomhero
// containers are customizable through the passed in vector.
async fn create_test_pod(
    name: String,
    arguments: &Vec<&str>,
    notification_config: Option<http_signals_sender::HttpNotificationConfig>,
) {
    let client = podman_client();

    // port_mappings is a list of port mappings we expose in the pod. the port 9000 is the port
    // oomhero exposes metrics while the port 9999 is the port where the workload pod exposes
    // endpoints for us to change its behavior (e.g. increase cpu usage).
    let port_mappings = vec![
        PortMapping {
            container_port: Some(9999),
            host_port: Some(9999),
            host_ip: None,
            protocol: None,
            range: None,
        },
        PortMapping {
            container_port: Some(9000),
            host_port: Some(9000),
            host_ip: None,
            protocol: None,
            range: None,
        },
    ];

    let mut volumes = vec![];
    if let Some(config) = notification_config {
        let content =
            serde_yaml::to_string(&config).expect("failed to encode http notification config");

        client
            .volumes()
            .create(
                &opts::VolumeCreateOpts::builder()
                    .name("oomhero_config")
                    .build(),
            )
            .await
            .expect("failed to create oomhero_config volume");

        let volume = client.volumes().get("oomhero_config");

        let info = volume
            .inspect()
            .await
            .expect("failed to inpect volume we just created");

        let mut dst = info.mountpoint.clone();
        dst.push_str("/config.yaml");
        fs::write(dst, content.as_bytes()).expect("failed to write config to file");

        volumes.push(NamedVolume {
            dest: Some(String::from("/etc/oomhero")),
            is_anonymous: Some(false),
            name: Some(String::from("oomhero_config")),
            options: None,
        })
    }

    let pod_create_opts = &opts::PodCreateOpts::builder()
        .name(name.clone())
        .volumes(volumes)
        .portmappings(port_mappings)
        .shared_namespaces(vec!["ipc", "net", "uts", "pid"])
        .infra_image("registry.k8s.io/pause:latest")
        .build();

    let workload_container_create_opts = &opts::ContainerCreateOpts::builder()
        .name("workload")
        .pod(name.clone())
        .resource_limits(workload_container_resource_limits().await)
        .image(WORKLOAD_IMAGE)
        .build();

    let oomhero_container_create_opts = &opts::ContainerCreateOpts::builder()
        .name("oomhero")
        .pod(name.clone())
        .resource_limits(oomhero_container_resource_limits())
        .add_capabilities(vec!["SYS_PTRACE"])
        .image(OOMHERO_IMAGE)
        .command(arguments)
        .build();

    let pod = client
        .pods()
        .create(&pod_create_opts)
        .await
        .expect("failed to create pod");

    client
        .containers()
        .create(&workload_container_create_opts)
        .await
        .expect("failed to create test image container");

    client
        .containers()
        .create(&oomhero_container_create_opts)
        .await
        .expect("failed to create test image container");

    pod.start().await.expect("failed to start pod");
}

// follow_container_logs dumps and then follows the whole logs from the provided container. This
// function only returns when the container dies.
async fn follow_container_logs(name: &str) {
    let container = podman_client().containers().get(name);
    let options = opts::ContainerLogsOpts::builder()
        .stdout(true)
        .stderr(true)
        .follow(true)
        .build();

    let mut log_stream = container.logs(&options);
    while let Some(chunk) = log_stream.next().await {
        let data = chunk.unwrap();
        print!("{}", String::from_utf8_lossy(&data));
    }
}

// attempt_test_pod_removal attempst to delete the test pod pointed by the provided name. Failures
// are ignored.
async fn attempt_test_pod_removal(name: String) {
    let pod = podman_client().pods().get(name);
    _ = pod.kill().await;
    _ = pod.remove().await;
}

// attempt_test_volume_removal attempst to delete the test volume pointed by the provided name.
// Failures are ignored.
async fn attempt_test_volume_removal(name: String) {
    let volume = podman_client().volumes().get(name);
    _ = volume.delete().await;
}

// wait_for_signals polls the /stats endpoint until the expected number of signals have been
// received.
async fn wait_for_signals(nr: i32) {
    for _ in 0..120 {
        let body = ureq::get("http://localhost:9999/stats")
            .call()
            .expect("failed to issue stats request")
            .body_mut()
            .read_to_string()
            .expect("failed to read issue stats request body");

        let stats: Stats = serde_json::from_str(&body).expect("failed to parse stats request body");

        if stats.signals_received == nr {
            info!("received signal nr {} as expected", nr);
            return;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    panic!("timeout waiting for signal nr {}", nr);
}

// test_basic_functionality test is very simple and needs to be improved. So far: it spawns a pod
// with both oomhero and a test workload application (source code under tests/workload). Both
// containers have memory and cpu restrictions. Once everything the pod is up we do a request to
// the workload appliation (/mem) so it immediately start to eat ram up, we wait until the
// application receives the signal. We repeat the same operation for cpu (/cpu). Nothing fancy
// here but it gets the basic functionality tested.
async fn test_basic_functionality() {
    // just in case we had a pod running from a failed previous attempt.
    attempt_test_pod_removal(String::from("oomhero_test_pod")).await;

    // create the pod with the two containers (three if we count the pause container). oomhero
    // is configured to warning on 80% and 90% for both memory usage and cpu pressure. Once
    // this call is back we know that the container is up and running;
    info!("creating test pod");
    create_test_pod(
        String::from("oomhero_test_pod"),
        &vec![
            "--warning=memory_usage > 80 || cpu_pressure_full_avg10 > 80 || io_pressure_full_avg10 > 50",
            "--critical=memory_usage > 90 || cpu_pressure_full_avg10 > 90 || io_pressure_full_avg10 > 80",
        ],
        None,
    )
    .await;

    // we follow container logs but we don't bother to join the task.
    _ = tokio::spawn(follow_container_logs("oomhero"));
    _ = tokio::spawn(follow_container_logs("workload"));

    // wait so we start showing the logs for both containers before the tests start. this make
    // debug easier (like in the case where the workload isn't starting by any reason).
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // here we issue a request to the workload application asking for it to start to eat cpu.
    // as the container is restricted to 10% of one CPU the pressure will start to grow, we
    // just need to monitor if it will receive the signal.
    info!("informing the test workload to start eating cpu");
    ureq::get("http://localhost:9999/cpu")
        .call()
        .expect("failed to ask for cpu burst");

    // wait for the signal to be sent by oomhero to the workload application.
    info!("waiting for the test workload to receive the first signal (cpu pressure)");
    wait_for_signals(1).await;
    info!("test workload informs that the cpu signal has been received");

    // we just wait a little bit before starting up the next test, memory consumption.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // we now rinse and repeat but this time assessing memory consumption.
    info!("informing the test workload to start eating memory");
    ureq::get("http://localhost:9999/mem")
        .call()
        .expect("failed to ask for mem burst");

    info!("waiting for the test workload to receive the second signal (mem usage)");
    wait_for_signals(2).await;
    info!("test workload informs that the memory signal has been received");

    attempt_test_pod_removal(String::from("oomhero_test_pod")).await;
}

// test_notify_command checks that the http notification is working.
async fn test_notify_command() {
    attempt_test_pod_removal(String::from("oomhero_test_pod")).await;
    attempt_test_volume_removal(String::from("oomhero_config")).await;

    info!("creating test pod");
    create_test_pod(
        String::from("oomhero_test_pod"),
        &vec![
            "--http-file-path=/etc/oomhero/config.yaml",
            "--warning=memory_usage > 60",
            "--critical=memory_usage > 90",
        ],
        Some(http_signals_sender::HttpNotificationConfig {
            url: String::from("http://localhost:9999/notification"),
            headers: vec![],
        }),
    )
    .await;

    _ = tokio::spawn(follow_container_logs("workload"));
    _ = tokio::spawn(follow_container_logs("oomhero"));

    // wait so we start showing the logs for both containers before the tests start. this make
    // debug easier (like in the case where the workload isn't starting by any reason).
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    info!("informing the test workload to start eating memory");
    ureq::get("http://localhost:9999/mem")
        .call()
        .expect("failed to ask for cpu burst");

    info!("waiting for the test workload to receive the signal");
    wait_for_signals(1).await;
    info!("test workload informs that the io signal has been received");

    attempt_test_pod_removal(String::from("oomhero_test_pod")).await;
    attempt_test_volume_removal(String::from("oomhero_config")).await;
}

// end_2_end is the entry point for the end to end tests. it calls one by one. nothing to see here
// other than the ordering. the logger is started on this function.
#[tokio::test]
async fn end_2_end() {
    let environment = env_logger::Env::new().default_filter_or("info");
    env_logger::Builder::from_env(environment).init();

    test_basic_functionality().await;
    test_notify_command().await;
}
