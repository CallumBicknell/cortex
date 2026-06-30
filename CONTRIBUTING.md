# Contributing to Cortex

Thank you for your interest in contributing to Cortex! We welcome contributions from the community.

## How to Contribute

1. Fork the repository on GitHub.
2. Clone your fork locally.
3. Create a new branch for your feature or bugfix.
4. Make your changes.
5. Ensure your code follows the project's coding standards.
6. Add tests for any new functionality.
7. Run the test suite to ensure everything passes.
8. Commit your changes using a clear, descriptive commit message.
9. Push your branch to your fork.
10. Open a pull request against the main branch of this repository.

## Development Setup

### Prerequisites

- Rust (stable toolchain)
- Python 3.8+
- Git

### Building

```bash
# Clone the repository
git clone https://github.com/yourusername/cortex.git
cd cortex

# Build the project
cargo build --release
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p cortex-core
```

### Code Formatting

We use `rustfmt` for Rust code formatting. Ensure your code is formatted before submitting a pull request.

```bash
cargo fmt
```

### Linting

We use `clippy` for linting. Run the linter to catch potential issues.

```bash
cargo clippy --all -- -D warnings
```

### Documentation

If your change affects the API, configuration, or user-facing behavior, please update the relevant documentation.

## Reporting Issues

Please use the GitHub issue tracker to report bugs or request features. When reporting a bug, include:

- A clear and descriptive title.
- Steps to reproduce the issue.
- Expected behavior vs. actual behavior.
- Any relevant logs or error messages.
- Information about your environment (OS, Rust version, etc.).

## Code Review

All contributions must be reviewed by at least one maintainer. Please be responsive to feedback and make requested changes promptly.

## License

By contributing to Cortex, you agree that your contributions will be licensed under the MIT License (or Apache-2.0 at your option).

## Questions?

If you have any questions, feel free to open an issue or reach out to the maintainers.

Happy coding!