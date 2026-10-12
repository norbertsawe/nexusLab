# Contributing to NexusLab

Bug reports, documentation improvements, and focused code changes are welcome.

## Reporting an issue

Include your NexusLab version, operating system, and the steps needed to
reproduce the problem. For a scripting issue, provide a small example and the
expected result. For acquisition issues, include the device type, sampling
rate, framing, and connection settings.

## Development

Clone the repository and install the prerequisites described in the README.
Before opening a pull request, run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

Keep each change focused on one problem. Explain the resulting behavior and
how you verified it. Include a test when changing numerical results, parsing,
file handling, or other behavior that needs a regression check.

GUI changes should be checked in both themes and in detached panels. Desktop
smoke scripts are described in [testing notes](docs/testing.md); they require
an interactive Windows desktop.

New device support should include the framing or protocol assumptions and a
repeatable way to test them. Use simulated input when a physical device is not
available.
