FROM golang
ADD . /src
WORKDIR /src
RUN GOOS=linux CGO_ENABLED=0 go build -ldflags="-w -s" -a -installsuffix cgo ./cmd/oomhero

FROM scratch 
COPY --from=0 /src/oomhero /
ENTRYPOINT [ "/oomhero" ]
