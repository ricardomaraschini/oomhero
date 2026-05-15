use nix::sys::stat;
use podman_api::models::LinuxBlockIo;
use podman_api::models::LinuxCpu;
use podman_api::models::LinuxMemory;
use podman_api::models::LinuxResources;
use podman_api::models::LinuxThrottleDevice;
use podman_api::models::PortMapping;
use podman_api::opts;
use podman_api::Podman;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;
use users::get_current_uid;

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
// use to make testing easier.
async fn workload_container_resource_limits() -> LinuxResources {
    let mut limits = LinuxResources {
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
    };

    // we only ingest io limits if the controller is enabled. if the user is using
    // root to run the tests then this is most likely be enabled. systemd does not
    // delegate the io controller to regular users.
    if io_controller_is_enabled().await {
        let (major, minor) = major_and_minor_numbers_for_podman_storage().await;
        limits.block_io = Some(LinuxBlockIo {
            throttle_write_iops_device: Some(vec![LinuxThrottleDevice {
                major: Some(major as i64),
                minor: Some(minor as i64),
                rate: Some(100),
            }]),
            leaf_weight: None,
            throttle_read_bps_device: None,
            throttle_read_iops_device: None,
            throttle_write_bps_device: None,
            weight: None,
            weight_device: None,
        });
    }

    limits
}

// major_and_minor_numbers_for_podman_storage finds out the device driver major and minor numbers
// for the device in which the containers have their temporary storage mounted. The container is
// then expected to execute io on this device, we can then restrict it.
async fn major_and_minor_numbers_for_podman_storage() -> (u64, u64) {
    let client = podman_client();
    let info = client
        .info()
        .await
        .expect("failed to read podman information");

    let path = info.store.unwrap().graph_root.unwrap();
    let device = stat::stat(path.as_str()).expect("failed to stat podman storage fs");
    let major = stat::major(device.st_dev);
    let minor = stat::minor(device.st_dev);

    // if this device is a partition we need to search for the parent device as we can't impose
    // io limits directly on the partition.
    if let Some(parent_data) = major_and_minor_numbers_for_parent(major, minor) {
        parent_data
    } else {
        (major, minor)
    }
}

// major_and_minor_numbers_for_parent returns the major and minor numbers for the device who is
// parent of the device identified by the provided major and minor. this is used to identify
// what is the disk in which a given partition is, we can't impose io limits in a partition, we
// need to impose on the whole disk.
fn major_and_minor_numbers_for_parent(major: u64, minor: u64) -> Option<(u64, u64)> {
    let partition_file = format!("/sys/dev/block/{}:{}/partition", major, minor);
    if !Path::new(&partition_file).exists() {
        return None;
    }

    let dev_file = format!("/sys/dev/block/{}:{}/../dev", major, minor);
    let dev = fs::read_to_string(&dev_file).expect("failed to read dev file");

    // format is <major>:<minor>
    let parts: Vec<&str> = dev.trim().split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let major: u64 = parts[0].parse().expect("failed to parse major number");
    let minor: u64 = parts[1].parse().expect("failed to parse minor number");
    Some((major, minor))
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
// /run/podman/podman.sock. This test can be ran as either root or regular user but the full
// coverage can only be achieved with root (systemd does not delegate some cgroup controllers to
// regular users).
fn podman_client() -> Podman {
    if get_current_uid() == 0 {
        return Podman::unix("/run/podman/podman.sock");
    }
    let runtime_dir = env::var("XDG_RUNTIME_DIR").expect("failed to read xdg runtime dir");
    let socket_path = format!("{}/podman/podman.sock", runtime_dir);
    Podman::unix(socket_path)
}

// io_controller_is_enabled returns true if the io controller is enabled. if it is disabled then
// some of the tests aren't going to run. if you are running the tests as root then you probably
// have it enabled (systemd does not delegate it to regular users though).
async fn io_controller_is_enabled() -> bool {
    let client = podman_client();
    let info = client
        .info()
        .await
        .expect("failed to read podman information");
    let controllers = info.host.unwrap().cgroup_controllers.unwrap();
    controllers.contains(&"io".to_string())
}

// create_test_pod will create a pod with three containers, one with the pause image, one with the
// test image (see tests/image directory) and one with the oomhero. The arguments to the oomhero
// containers are customizable through the passed in vector.
async fn create_test_pod(name: String, arguments: &Vec<&str>) {
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

    let pod_create_opts = &opts::PodCreateOpts::builder()
        .name(name.clone())
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

// attempt_test_pod_removal attempst to delete the test pod pointed by the provided name. Failures
// are ignored.
async fn attempt_test_pod_removal(name: String) {
    let pod = podman_client().pods().get(name);
    _ = pod.kill().await;
    _ = pod.remove().await;
}

// end_2_end test is very simple and needs to be improved. So far: it spawns a pod with both
// oomhero and a test workload application (source code under tests/workload). Both containers
// have memory and cpu restrictions. Once everything the pod is up we do a request to the
// workload appliation (/mem) so it immediately start to eat ram up, we wait until the
// application receives the signal. We repeat the same operation for cpu (/cpu). Nothing fancy
// here but it gets the basic functionality tested.
#[tokio::test]
async fn end_2_end() {
    // just in case we had a pod running from a failed previous attempt.
    attempt_test_pod_removal(String::from("oomhero_test_pod")).await;

    if !io_controller_is_enabled().await {
        println!("*****************************************************************");
        println!("* IO TESTS WILL BE SKIPPED BECAUSE THE CONTROLLER ISN'T ENABLED *");
        println!("* YOU MAY WANT TO RUN THIS TEST AS ROOT OR JUST DELEGATE TO CI  *");
        println!("*****************************************************************");
    }

    // create the pod with the two containers (three if we count the pause container). oomhero
    // is configured to warning on 80% and 90% for both memory usage and cpu pressure. Once
    // this call is back we know that the container is up and running;
    println!("creating test pod");
    create_test_pod(
        String::from("oomhero_test_pod"),
        &vec![
            "--memory-usage-warning=80",
            "--memory-usage-critical=90",
            "--cpu-pressure-warning=80",
            "--cpu-pressure-critical=90",
            "--io-pressure-warning=50",
            "--io-pressure-critical=80",
        ],
    )
    .await;

    // here we issue a request to the workload application asking for it to start to eat cpu.
    // as the container is restricted to 10% of one CPU the pressure will start to grow, we
    // just need to monitor if it will receive the signal.
    println!("informing the test workload to start eating cpu");
    let client = reqwest::Client::new();
    client
        .get("http://localhost:9999/cpu")
        .send()
        .await
        .expect("failed to send /cpu request");

    // wait for the signal to be sent by oomhero to the workload application.
    println!("waiting for the test workload to receive the first signal (cpu pressure)");
    wait_for_signals(&client, 1).await;
    println!("test workload informs that the cpu signal has been received");

    // we just wait a little bit before starting up the next test, memory consumption.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // we now rinse and repeat but this time assessing memory consumption.
    println!("informing the test workload to start eating memory");
    client
        .get("http://localhost:9999/mem")
        .send()
        .await
        .expect("failed to send /mem request");

    println!("waiting for the test workload to receive the second signal (mem usage)");
    wait_for_signals(&client, 2).await;
    println!("test workload informs that the memory signal has been received");

    if !io_controller_is_enabled().await {
        attempt_test_pod_removal(String::from("oomhero_test_pod")).await;
        return;
    }

    // we just wait a little bit before starting up the next test, io consumption.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // we now rinse and repeat but this time assessing memory consumption.
    println!("informing the test workload to start doing io");
    client
        .get("http://localhost:9999/io")
        .send()
        .await
        .expect("failed to send /io request");

    println!("waiting for the test workload to receive the third signal (io usage)");
    wait_for_signals(&client, 3).await;
    println!("test workload informs that the io signal has been received");

    attempt_test_pod_removal(String::from("oomhero_test_pod")).await;
}

// wait_for_signals polls the /stats endpoint until the expected number of signals have been
// received.
async fn wait_for_signals(client: &reqwest::Client, nr: i32) {
    for _ in 0..120 {
        let stats: Stats = client
            .get("http://localhost:9999/stats")
            .send()
            .await
            .expect("failed to get stats")
            .json()
            .await
            .expect("failed to parse stats");

        if stats.signals_received == nr {
            println!("received signal nr {} as expected", nr);
            return;
        }

        println!("sig received {}, expected {}", stats.signals_received, nr);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    panic!("timeout waiting for signal nr {}", nr);
}
