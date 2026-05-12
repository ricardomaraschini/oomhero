IMAGE ?= ghcr.io/ricardomaraschini/oomhero
TAG ?= latest

.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release

.PHONY: image-build
image-build:
	docker build -t $(IMAGE):latest -f Containerfile .
	docker tag $(IMAGE):latest $(IMAGE):$(TAG)

.PHONY: image-push
image-push:
	docker push $(IMAGE):$(TAG)

.PHONY: image-sign
image-sign:
	cosign sign --yes $(IMAGE):$(TAG)

.PHONY: image-build-push
image-build-push: image-build image-push

.PHONY: test
test:
	timeout 5m cargo test

.PHONY: test-verbose
test-verbose:
	timeout 5m cargo test -- --nocapture
