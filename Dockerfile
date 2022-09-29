FROM eu.gcr.io/talon-farm2/talon-one/docker-go-node/master:d54d4cb60b62f36d43156d01e487debc1553259c
ADD . /src
WORKDIR /src
RUN GOOS=linux CGO_ENABLED=0 go build -ldflags="-w -s" -a -installsuffix cgo ./cmd/oomhero

FROM scratch
COPY --from=0 /src/oomhero /
ENTRYPOINT [ "/oomhero" ]
