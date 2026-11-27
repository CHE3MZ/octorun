<img src="assets/logo.png" alt="Octorun Logo" width="376" />

# **octorun**

[![CI checks](https://img.shields.io/github/actions/workflow/status/CHE3MZ/octorun/ci.yml?style=for-the-badge&logo=github&logoColor=99A0A8&labelColor=373F46&label=CI&color=32CA55)](https://github.com/CHE3MZ/octorun/actions/workflows/ci.yml)

### octorun is a tool that lets you deploy GitHub Actions local runners at blazingly fast speeds!

<img src="assets/placeholder.png" alt="Demo gif" width="256" />

# Installation

<img src="assets/platforms.png" alt="platforms" width="376" />

### octorun works on all the platforms that the [github actions self hosted runner supports!](https://docs.github.com/en/actions/reference/runners/self-hosted-runners)

## Package managers

### Brew

```sh
brew install CHE3MZ/tools/octorun
```

## Cargo

### Cargo install

```sh
cargo install --locked octorun
```

### Cargo binstall

```sh
cargo binstall octorun
```

## From the installation scripts

### MacOS & Linux

```powershell
curl -LsSf https://raw.githubusercontent.com/CHE3MZ/octorun/master/scripts/install/install.sh | sh
```

### Windows

```powershell
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

to stop the runner, ctrl+C the current runner terminal session or run the `stop` command:

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
octorun help <COMMAND>
```

# Development

To get started, install [go-task](https://taskfile.dev/) (A.K.A [taskfile](https://taskfile.dev/)), you will need it to run a lot of the local CI jobs.

<img src="assets/actions.png" alt="notice" width="64" />

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

this project is licensed under the [MIT License.](LICENSE)
