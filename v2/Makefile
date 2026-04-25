IMAGE ?= docker.io/ricardomaraschini/oomhero:v2

.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release

.PHONY: image-build
image-build:
	docker build -t $(IMAGE) -f Containerfile .

.PHONY: image-push
image-push:
	docker push $(IMAGE)

.PHONY: image-build-push
image-build-push: image-build image-push
