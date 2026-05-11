IMAGE ?= ghcr.io/ricardomaraschini/oomhero
VERSION ?= v$(shell cargo pkgid | cut -d# -f2)

.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release

.PHONY: image-build
image-build:
	docker build -t $(IMAGE):latest -f Containerfile .
	docker tag $(IMAGE):latest $(IMAGE):$(VERSION)

.PHONY: image-push
image-push:
	docker push $(IMAGE):latest
	docker push $(IMAGE):$(VERSION)

.PHONY: image-sign
image-sign:
	cosign sign --yes $(IMAGE):latest
	cosign sign --yes $(IMAGE):$(VERSION)

.PHONY: image-build-push
image-build-push: image-build image-push

.PHONY: test
test:
	timeout 5m cargo test

.PHONY: test-verbose
test-verbose:
	timeout 5m cargo test -- --nocapture
