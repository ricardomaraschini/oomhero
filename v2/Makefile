.PHONY: build
build:
	cargo build --release

.PHONY: image-build
image-build:
	docker build -t docker.io/ricardomaraschini/oomhero:v2 -f Containerfile .

.PHONY: image-push
image-push:
	docker push docker.io/ricardomaraschini/oomhero:v2

.PHONY: image-build-push
image-build-push: image-build image-push
