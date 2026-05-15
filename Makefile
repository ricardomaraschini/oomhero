IMAGE ?= ghcr.io/ricardomaraschini/oomhero
TAG ?= latest
IMAGEFULL = $(IMAGE):$(TAG)

.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release

.PHONY: image-build
image-build:
	podman build -t $(IMAGEFULL) .

.PHONY: image-push
image-push:
	podman push $(IMAGEFULL)

.PHONY: lint
lint:
	cargo clippy -- -D warnings

.PHONY: image-build-push
image-build-push: image-build image-push

.PHONY: test
test: test-workload-image-build image-build
	timeout 5m cargo test

.PHONY: test-verbose
test-verbose: test-workload-image-build image-build
	timeout 5m cargo test -- --nocapture

.PHONY: test-workload-image-build
test-workload-image-build:
	podman build -t test-workload tests/workload

# the next recipe is tailored to run things as root. this should not be
# needed locally but is paramount for running the full end to end tests.
# the runner does not have cargo installed for the root user, this is
# a hack. we build the  needed images and then run the tests.
.PHONY: test-verbose-as-root
test-verbose-as-root:
	sudo podman build -t test-workload tests/workload
	sudo podman build -t $(IMAGEFULL) .
	CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="sudo" timeout 5m cargo test -- --nocapture
