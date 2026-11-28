<img src="assets/logo.png" alt="Octorun Logo" width="376" />

# octorun

[![CI checks](https://img.shields.io/github/actions/workflow/status/CHE3MZ/octorun/ci.yml?style=for-the-badge&logo=github&logoColor=99A0A8&labelColor=373F46&label=CI&color=32CA55)](https://github.com/CHE3MZ/octorun/actions/workflows/ci.yml)
[![Website](https://img.shields.io/website?url=https%3A%2F%2Fche3mz.github.io%2Foctorun%2F&style=for-the-badge&logo=readthedocs&logoColor=99A0A8&labelColor=373F46&label=Website&color=32CA55)](https://che3mz.github.io/octorun/)
[![Crates.io](https://img.shields.io/crates/v/octorun?style=for-the-badge&logo=rust&logoColor=99A0A8&labelColor=373F46&label=Crates.io&color=32CA55)](https://crates.io/crates/octorun)
[![Coverage Status](https://img.shields.io/coveralls/github/CHE3MZ/octorun?style=for-the-badge&logo=coveralls&logoColor=99A0A8&labelColor=373F46&label=Coverage&color=32CA55)](https://coveralls.io/github/CHE3MZ/octorun?branch=master)

### octorun is a tool that lets you deploy GitHub Actions local runners at blazingly fast speeds!

<img src="assets/demo.gif" alt="Demo gif" width="810" />

# Installation

<img src="assets/platforms.png" alt="platforms" width="376" />

### octorun works on all the platforms that the [GitHub Actions self hosted runner supports](https://docs.github.com/en/actions/reference/runners/self-hosted-runners)!

## Package managers

<a href="https://github.com/CHE3MZ/homebrew-tools/blob/master/Formula/octorun.rb">
  <img src="assets/homebrew.png" alt="homebrew" width="64" />
</a>

### Brew

```sh
brew install CHE3MZ/tools/octorun
```
<a href="https://crates.io/crates/octorun">
  <img src="assets/cargo.png" alt="cargo" width="64" />
</a>

### Cargo install

```sh
cargo install --locked octorun
```

### Cargo binstall

```sh
cargo binstall octorun
```

<a href="https://github.com/CHE3MZ/octorun/releases/latest">
  <img src="assets/github.png" alt="github" width="64" />
</a>

## From the installation scripts

### MacOS & Linux

```sh
curl -LsSf https://raw.githubusercontent.com/CHE3MZ/octorun/master/scripts/install/install.sh | sh
```

### Windows

```sh
irm https://raw.githubusercontent.com/CHE3MZ/octorun/master/scripts/install/install.ps1 | iex
```

# Usage

<img src="assets/octocat.png" alt="notice" width="184" />

To get started run the `create` command to create a new runner in the current directory:

```sh
octorun create
```

then run the `start` command to get it up and running:

```sh
octorun start
```

to stop the runner, Ctrl+C the current runner terminal session or run the `stop` command:

```sh
octorun stop
```

to remove the runner run the `remove` command:

```sh
octorun remove
```

if you want temporary runners run the `temp` command:

```sh
octorun temp
```

for more detailed help text run the `help` command:

```sh
octorun help
```

# CI

<img src="assets/ci.png" alt="actions" width="228" />

### If you want to create new empty runners that are **NOT** configured, use the `--no-configuration` flag with the `create` command:

```sh
octorun create --no-configuration
```

# Development

<img src="assets/development.png" alt="development" width="210" />

To get started, install [go-task](https://taskfile.dev/) (a.k.a [taskfile](https://taskfile.dev/)), you will need it to run a lot of the local CI jobs.


## Building

```
task build
```

## Ops

```
task ops
```

# License

<img src="assets/notice.png" alt="notice" width="196" />

Huge shoutout to [Dedac](https://github.com/Dedac/) for the project https://github.com/Dedac/gh-runner which this project is based on.

this project is licensed under the [MIT License](LICENSE).
