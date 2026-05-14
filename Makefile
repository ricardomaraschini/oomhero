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
