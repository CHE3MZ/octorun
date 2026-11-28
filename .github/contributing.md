# Contributing

Thanks for helping with octorun. Fork the repo, make a focused branch, run the checks, then open a pull request.

## Setup

You need Rust 1.90 or newer, Git, and optionally [go-task](https://taskfile.dev/) for the repository's task shortcuts.

Fork the project on GitHub, then clone your fork:

```sh
git clone https://github.com/OWNER/octorun.git
cd octorun
```

If you want CI to run on your fork, open the Actions tab and enable workflows there. The upstream CI also runs when you open the pull request.

## Make a change

Create a branch off `master`:

```sh
git switch -c fix-something
```

Use dashes in the branch name. A prefix like `fix-`, `feat-`, `ci-`, or `docs-` makes the intent clear. Do not commit to `master`.

Keep the diff focused. If the change needs a new dependency, explain why in the pull request.

## Checks

Run the tests and linters before pushing:

```sh
cargo test
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
```

If you have go-task, `task ops` runs the repository's local checks, including cargo-deny, cargo-audit, cargo-machete, taplo, shellcheck, actionlint, and zizmor. Some of those tools need to be installed first.

CI runs the tests on Linux, macOS, and Windows. Keep platform-specific code behind `cfg!` or `#[cfg]` so it builds everywhere.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org) for commits and for the pull request title:

```text
fix: stop the download bar from hanging
feat: add --no-configuration to create
ci: upload coverage to coveralls
```

Keep the subject short. Put the reason in the body when it is not obvious.

## Open a pull request

1. Push your branch to your fork.
2. Open a pull request against `CHE3MZ/octorun`.
3. Fill in the template. Use the same conventional prefix in the title as in your commits.
4. Wait for CI. Fix anything that fails before asking for review.

Small, focused pull requests with tests are easier to review than large ones.