# Getting Started

Three ways in: a dragon on your profile README, the command line, and the library.
Each stands alone.

## On Your Profile README

GitHub shows the README of the repository named after your username, such as
`octocat/octocat`, on your profile page. To give it a dragon that redraws every
night:

1. Copy [examples/profile-workflow.yml](../examples/profile-workflow.yml) into
   that repository as `.github/workflows/dragon.yml`.
2. Embed the dragon in its `README.md`:

   ```html
   <img src="dragon/dragon.gif" width="256" alt="My dragon" />
   ```

   The GIF is drawn at exactly twice the dragon's pixel size, so `width="256"`
   shows it at native size without blurring.

3. Run the workflow once from the repository's Actions tab, so the first dragon
   appears without waiting for midnight.

The workflow runs just after midnight UTC, so each dragon shows the day that just
ended, and again whenever the workflow file changes. Its `git-dragon` step is this
repository's Action. It collects your public activity with the workflow's
built-in token and renders `dragon.gif`, `dragon.png`, and `preview.html` into `dragon/`.
The Action never commits. The workflow's last step commits `dragon/dragon.gif` only
when it changed, so an unchanged dragon adds no commit.

| Input        | Default              | Meaning                                                   |
| ------------ | -------------------- | --------------------------------------------------------- |
| `login`      | The repository owner | Account whose dragon to draw                                 |
| `body`       | `emerald`            | `emerald`, `crimson`, `azure`, `obsidian`, `gold`, or `silver`   |
| `output-dir` | `dragon`                | Folder that receives `dragon.gif`, `dragon.png`, `preview.html` |
| `token`      | The built-in token   | Token for reading the account's public data               |

To change the body, edit `body` in your copy of the workflow; pushing the change
runs the workflow and redraws the dragon. If you change `output-dir`, change the
commit step's path and the README image to match. The example uses
`Saransh1220/git-dragon@v1`, which moves to each compatible release; pin a full
release tag to choose when you update.

GitHub pauses scheduled workflows in a public repository after 60 days without
activity such as a push. A dragon that stops changing during a long quiet stretch
adds no commits, so its schedule can pause and does not resume on its own.
Re-enable it from the Actions tab or with `gh workflow enable dragon.yml`.

## From the Command Line

Install one of two ways. With Rust at or above the `rust-version` in
[Cargo.toml](../Cargo.toml):

```sh
cargo install git-dragon --locked
```

Or download the archive for your platform from the
[releases page](https://github.com/Saransh1220/git-dragon/releases), a `.tar.gz`
for Linux and macOS or a `.zip` for Windows. Each archive has a `.sha256` file
beside it; check the download with `shasum -a 256 -c` on macOS, `sha256sum -c`
on Linux, or `certutil -hashfile` on Windows, then put `git-dragon` on your `PATH`.
macOS may refuse to open a binary it downloaded;
`xattr -d com.apple.quarantine git-dragon` clears that flag.

Collection goes through the [GitHub CLI](https://cli.github.com), so install it
and sign in once with `gh auth login`. Then:

```sh
git-dragon collect octocat --output snapshot.json
git-dragon render snapshot.json --output-dir dragon --body azure
```

Open `dragon/preview.html` in a browser to see the GIF and the still on GitHub's
light and dark page colors. `--body` takes `emerald`, the default, `crimson`,
`azure`, `obsidian`, `gold`, or `silver`.

`collect` reads only public data: the account's public repositories, the public
repositories it has contributed to, GitHub's color for each language, and its
daily contributions from the account's creation through the last complete UTC
day. It writes the snapshot to standard output, or with `--output` replaces the
file only after collection succeeds. In GitHub Actions, `GH_TOKEN` set to the
workflow's token is enough. `render` draws everything in memory first and
replaces each file through a temporary file, so a failed run keeps the previous
files. Both print errors to standard error and exit 1; invalid usage exits 2.
`git-dragon --help` prints the full usage and `git-dragon --version` the version.

## As a Library

The same crate is a library, documented on [docs.rs](https://docs.rs/git-dragon).
Add it beside `serde_json`, which reads the snapshot:

```sh
cargo add git-dragon serde_json
```

The pipeline runs one way. Read or collect an `AccountSnapshot`, derive the
dragon's development, choose the day's behavior, render frames, and encode them:

```rust
use git_dragon::behavior::select_behavior;
use git_dragon::coat::DragonBody;
use git_dragon::history::AccountSnapshot;
use git_dragon::dragon::derive_dragon_development;
use git_dragon::motion::frame_count;
use git_dragon::output::{encode_gif, encode_png};
use git_dragon::render::{render_dragon, render_flight_frame};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let snapshot_json = std::fs::read_to_string("snapshot.json")?;
    let account_snapshot: AccountSnapshot = serde_json::from_str(&snapshot_json)?;
    let dragon_development = derive_dragon_development(&account_snapshot)?;
    let dragon_body = DragonBody::Asagi;
    let behavior = select_behavior(&dragon_development);
    let flight_frames: Vec<_> = (0..frame_count(behavior))
        .map(|frame_index| render_flight_frame(&dragon_development, dragon_body, behavior, frame_index))
        .collect();
    std::fs::write("dragon.gif", encode_gif(&flight_frames)?)?;
    std::fs::write("dragon.png", encode_png(&render_dragon(&dragon_development, dragon_body))?)?;
    Ok(())
}
```

To collect in-process instead of reading a file, call
`collect::collect_account_snapshot` with `collect::GhCli` and today's UTC date.
Everything after collection is deterministic and performs no I/O, so the same
snapshot always draws the same dragon. Breaking changes to the API bump the major
version.
