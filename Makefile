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
	docker build -t $(IMAGEFULL) -f Containerfile .

.PHONY: image-push
image-push:
	docker push $(IMAGEFULL)

.PHONY: image-sign
WITHSHA=$(shell docker inspect -f '{{index .RepoDigests 0}}' $(IMAGEFULL))
image-sign:
	cosign sign --yes $(WITHSHA)

.PHONY: image-build-push
image-build-push: image-build image-push

.PHONY: image-build-push-sign
image-build-push-sign: image-build image-push image-sign

.PHONY: test
test:
	timeout 5m cargo test

.PHONY: test-verbose
test-verbose:
	timeout 5m cargo test -- --nocapture
