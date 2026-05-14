# Contributing to OOMHero
First off, thank you for considering contributing to OOMHero. It's people like
you that make OpenSource such an amazing thing.

## Code of Conduct
By participating in this project, you are expected to uphold professional and
respectful conduct in all interactions.

## Getting Started

### Prerequisites
To build and test OOMHero, you will need:

- **Rust**: The latest stable version of Rust (edition 2024).
- **Podman**: Used for building container images and running integration tests.
- **Linux**: OOMHero relies on Linux-specific features like Pressure Stall
  Information (PSI) and `procfs`.

### Building from Source
You can build the project using standard Cargo commands or the provided
`Makefile`:

```bash
# Using Makefile
make build

# Using Cargo
cargo build
```

For release builds:

```bash
# Using Makefile
make release

# Using Cargo
cargo build --release
```

## Development Workflow

### Testing
Testing is a critical part of OOMHero. We aim to have both unit tests and
integration tests. To run all tests:

```bash
make test
```

For more verbose output:

```bash
make test-verbose
```

#### Integration Tests
Integration tests build a `test-workload` container image and require a running
Podman socket. Ensure the socket is available:
```bash
systemctl --user start podman.socket
```

#### Mocking System Data
Many tests use mock `procfs` and `cgroup` data located in `tests/data/`. When
adding features that read new system files, please add corresponding mock data
to ensure tests are reproducible and don't depend on the host system's state.

### Coding Standards
We follow standard Rust idioms and formatting.

- **Formatting**: Always format your code with `rustfmt` before submitting.
  ```bash
  cargo fmt
  ```
- **Linting**: We use `clippy` to catch common mistakes and improve code quality.
  ```bash
  make lint
  ```

### Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/). This
helps us automate our release process and keep a clean history.


## Submitting Changes
1.  **Fork the repository** and create your branch from `main`.
2.  **Make your changes**. Ensure that you add tests for any new features or
    bug fixes.
3.  **Run tests and linters**. Ensure `make test`, `cargo fmt`, and `make lint`
    pass.
4.  **Open a Pull Request**. Provide a clear description of the changes and
    link to any relevant issues.

### Pull Request Guidelines
- Keep PRs focused. If you have multiple unrelated changes, please open separate PRs.
- Ensure CI passes. Every PR triggers a GitHub Action that runs `make test-verbose`.
- Update documentation in `README.md` if you've added or changed configuration options.
