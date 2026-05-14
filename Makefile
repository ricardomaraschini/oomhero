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

.PHONY: image-sign
WITHSHA=$(shell podman inspect -f '{{index .RepoDigests 0}}' $(IMAGEFULL))
image-sign:
	cosign sign --yes $(WITHSHA)

.PHONY: image-build-push
image-build-push: image-build image-push

.PHONY: image-build-push-sign
image-build-push-sign: image-build image-push image-sign

.PHONY: test
test: test-workload-image-build image-build
	timeout 5m cargo test

.PHONY: test-verbose
test-verbose: test-workload-image-build image-build
	timeout 5m cargo test -- --nocapture

.PHONY: test-workload-image-build
test-workload-image-build:
	podman build -t test-workload tests/workload
