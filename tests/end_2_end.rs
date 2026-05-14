use podman_api::Podman;
use podman_api::models::LinuxCpu;
use podman_api::models::LinuxMemory;
use podman_api::models::LinuxResources;
use podman_api::models::PortMapping;
use podman_api::opts;
use std::env;

// WORKLOAD_IMAGE is the image that simulates an actual workload on a cluster. It is the
// application that is monitored by the oomhero container, receives signals and  reacts
// to them. During e2e the image under tests/image is used. For tests to work this image
// is expected to be already present in the podman storage.
const WORKLOAD_IMAGE: &str = "test-workload";

// OOMHERO_IMAGE is the oomhero version we are testing. This image is expected to be present
// in the podman storage prior to run the tests. Before running the test make sure you
// built the image.
const OOMHERO_IMAGE: &str = "ghcr.io/ricardomaraschini/oomhero";

// WORKLOAD_CONTAINER_RESOURCE_LIMITS limits the amount of resources that the test workload
// container can use.
const WORKLOAD_CONTAINER_RESOURCE_LIMITS: LinuxResources = LinuxResources {
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

// OOMHERO_CONTAINER_RESOURCE_LIMITS limits the amount of resources our test oomhero container can
// use during the test execution.
const OOMHERO_CONTAINER_RESOURCE_LIMITS: LinuxResources = LinuxResources {
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
};

// podman_client returns a client pointing to the podman socket. The socket is expected to be under
// $XDG_RUNTIME_DIR/podman/podman.sock.
fn podman_client() -> Podman {
    let runtime_dir = env::var("XDG_RUNTIME_DIR").expect("failed to read xdg runtime dir");
    let socket_path = format!("{}/podman/podman.sock", runtime_dir);
    Podman::unix(socket_path)
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
        .resource_limits(WORKLOAD_CONTAINER_RESOURCE_LIMITS)
        .image(WORKLOAD_IMAGE)
        .build();

    let oomhero_container_create_opts = &opts::ContainerCreateOpts::builder()
        .name("oomhero")
        .pod(name.clone())
        .resource_limits(OOMHERO_CONTAINER_RESOURCE_LIMITS)
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
    _ = pod.stop().await;
    _ = pod.remove().await;
}

#[tokio::test]
async fn end_2_end() {
    attempt_test_pod_removal(String::from("memory_pressure")).await;

    create_test_pod(
        String::from("memory_pressure"),
        &vec![
            "--memory-usage-warning",
            "80",
            "--memory-usage-critical",
            "90",
            "--cpu-pressure-warning",
            "80",
            "--cpu-pressure-critical",
            "90",
        ],
    )
    .await;

    // attempt_test_pod_removal(String::from("memory_pressure")).await;
}
