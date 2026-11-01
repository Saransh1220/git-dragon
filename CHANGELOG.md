# Changelog

Notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-11-01

### Changed

- Forked from git-koi: the koi is now a baby dragon seen from the side. It has bat
  wings, legs, horns, and a spade-tipped tail that all grow with contributions, a pale
  plated belly, a ridge of back spines, and an amber eye. It flies in place instead
  of swimming laps: the wings beat and the body bobs, with harder beats and a surge
  when darting, lazy beats when resting, folded wings when asleep, and fire breathing
  instead of surface feeding. The bodies are emerald, crimson, azure, obsidian, gold,
  and silver (`--body` names changed to match).

## [1.0.2] - 2026-10-31

### Fixed

- `git-koi collect` requests again any contribution days GitHub leaves out of an
  answer, instead of failing with "missing contribution date". GitHub left out
  days for some accounts whose history crosses a February 29 (#52).

## [1.0.1] - 2026-10-28

### Changed

- The Action is listed as `git-profile-koi`, since a GitHub user already holds
  the name `git-koi`, and its description fits the Marketplace limit.

## [1.0.0] - 2026-10-28

### Added

- `git-koi collect`, which gathers an account's public snapshot through the
  GitHub CLI, and `--help` and `--version` on the command.
- `git-koi render`, which draws the day's koi as a looping GIF, a still PNG, and
  a preview page, in six bodies.
- A GitHub Action that downloads the release binary and runs both inside a
  profile repository's workflow.
- The `git-koi` library: account history, koi development, behavior, motion,
  markings, rendering, and encoding.

[Unreleased]: https://github.com/yalongwastaken/git-koi/compare/v1.0.2...HEAD
[1.0.2]: https://github.com/yalongwastaken/git-koi/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/yalongwastaken/git-koi/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/yalongwastaken/git-koi/releases/tag/v1.0.0
