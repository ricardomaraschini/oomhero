use podman_api::Podman;
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

    let workload_port_mappings = vec![PortMapping {
        container_port: Some(9999),
        host_port: Some(9999),
        host_ip: None,
        protocol: None,
        range: None,
    }];

    let pod_create_opts = &opts::PodCreateOpts::builder()
        .name(name.clone())
        .portmappings(workload_port_mappings)
        .shared_namespaces(vec!["ipc", "net", "uts", "pid"])
        .infra_image("registry.k8s.io/pause:latest")
        .build();

    let workload_container_create_opts = &opts::ContainerCreateOpts::builder()
        .name("workload")
        .pod(name.clone())
        .image(WORKLOAD_IMAGE)
        .build();

    let oomhero_container_create_opts = &opts::ContainerCreateOpts::builder()
        .name("oomhero")
        .pod(name.clone())
        .cpu_quota(1)
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
            "90",
            "--memory-usage-critical",
            "96",
            "--loop-interval",
            "1s",
        ],
    )
    .await;
}
