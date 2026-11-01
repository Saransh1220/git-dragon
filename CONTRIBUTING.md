# Contributing

Thanks for helping. This page says how to build the project, how to check a
change, and what a pull request looks like.

## Requirements

- Rust with Cargo, rustfmt, and Clippy, at least the `rust-version` in
  [Cargo.toml](Cargo.toml). Install it with [rustup](https://rustup.rs).
- Node.js, which runs Prettier for documentation and configuration files.
- The [GitHub CLI](https://cli.github.com), authenticated, to run
  `git-dragon collect` against a real account.

## Build

```sh
cargo build --release --locked
./target/release/git-dragon --help
```

`--locked` keeps `Cargo.lock` as committed.

## Gates

Every pull request passes six gates: rustfmt, Clippy with warnings denied, the
tests, a release build, rustdoc with warnings denied, and Prettier over the
whole tree. The exact commands live in the CI workflow,
[.github/workflows/ci.yml](.github/workflows/ci.yml); run them locally before
pushing. To fix formatting rather than check it:

```sh
cargo fmt --all
npx prettier@3.9.10 --write .
```

## Pull Requests

- One topic per pull request. Independent changes travel separately.
- The commit subject is `area: lowercase imperative phrase stating the change`,
  where the area is the module or workflow, such as `render:` or `ci:`. The body
  says why in a few lines.
- Tests sit at public boundaries and fail under the bug they name. They do not
  pin incidental details.
- Documentation changes with the behavior it describes: doc comments, the
  README, and the guide in `docs/`. A change a user would notice gets a line
  under Unreleased in [CHANGELOG.md](CHANGELOG.md).
- Formatting changes touch only the lines the change touches.

## Reviewing Rendering Changes

The dragon library shows every body across growth samples, language mixes, and
streak marks, as transparent PNG and SVG pairs plus light and dark comparison
sheets. Generate it from the repository root, then open `library/README.md`; Git
ignores the generated files, and CI attaches a fresh library to every pull
request as the `dragon-library` artifact.

```sh
cargo run --locked --example generate-library -- library
```

When a change alters how the dragon looks, copy `library/bodies-light.png` and
`library/bodies-dark.png` into `docs/` so the README's sheet matches the code.

## Releasing

A release is a version tag, and the release workflow does the rest.

1. Set `version` in [Cargo.toml](Cargo.toml) and run `cargo build --locked` so
   `Cargo.lock` follows. Move the Unreleased entries in
   [CHANGELOG.md](CHANGELOG.md) under a heading for the version with the day's
   date, and keep the link references at the bottom current.
2. Merge that through a pull request, then tag the merge commit `vX.Y.Z` and
   push the tag.

The workflow checks the tag against the manifest and the changelog, builds the
binaries for Linux, macOS, and Windows, publishes the GitHub Release with the
changelog section as its notes, and runs the Action against that release. Only
once that passes does it move the major tag such as `v1`, so the Action follows
the release, and publish the crate through crates.io trusted publishing, the two
steps that cannot be undone. Every step skips what already exists, so rerunning
a failed release is safe. The Action downloads the release whose version its own
`Cargo.toml` names, so a release tag is the only thing it can be referenced at.

## Issues and Security

Open bugs and feature requests through the issue forms. Report vulnerabilities
privately as [SECURITY.md](SECURITY.md) describes, never in a public issue.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).
