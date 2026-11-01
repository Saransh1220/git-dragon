use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use chrono::{DateTime, NaiveDate};
use git_dragon::behavior::{Behavior, select_behavior};
use git_dragon::coat::DragonBody;
use git_dragon::collect::{GhCli, collect_account_snapshot};
use git_dragon::history::AccountSnapshot;
use git_dragon::dragon::derive_dragon_development;
use git_dragon::motion::frame_count;
use git_dragon::output::{encode_gif, encode_png, encode_preview_html};
use git_dragon::render::{DragonFrame, render_dragon, render_flight_frame};

const USAGE: &str = "Usage: git-dragon collect LOGIN [--output FILE]
       git-dragon render SNAPSHOT --output-dir DIR [--body BODY]
       git-dragon --help | --version
Collect LOGIN's public GitHub snapshot as JSON through `gh api`: public owned and
contributed repositories, and daily contributions from the account's creation
through yesterday (UTC), the last complete day.
Writes to standard output, or replaces FILE only after collection succeeds.
Requires an authenticated GitHub CLI; in GitHub Actions, set GH_TOKEN to github.token.
Render a collected SNAPSHOT into DIR: dragon.gif flies the behavior chosen for its
observation date, dragon.png is the still dragon, and preview.html shows both on light and
dark backgrounds. BODY is emerald (the default), crimson, azure, obsidian, gold, or silver.
The files are replaced only after all of them render.
Errors go to stderr and exit 1; invalid usage exits 2.
Examples: git-dragon collect octocat --output snapshot.json
          git-dragon render snapshot.json --output-dir dragon --body azure";

/// Each body's name on the command line.
const BODY_NAMES: [(&str, DragonBody); 6] = [
    ("emerald", DragonBody::Emerald),
    ("obsidian", DragonBody::Obsidian),
    ("azure", DragonBody::Azure),
    ("crimson", DragonBody::Crimson),
    ("gold", DragonBody::Gold),
    ("silver", DragonBody::Silver),
];

fn main() -> ExitCode {
    let program_arguments: Vec<OsString> = env::args_os().skip(1).collect();
    let argument_texts: Vec<Option<&str>> = program_arguments
        .iter()
        .map(|argument| argument.to_str())
        .collect();
    match argument_texts.as_slice() {
        [Some("--help")] | [Some("collect" | "render"), Some("--help")] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        [Some("--version")] => {
            println!("git-dragon {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [Some("collect"), Some(login)] if !login.starts_with('-') => run_collect(login, None),
        [Some("collect"), Some(login), Some("--output"), _] if !login.starts_with('-') => {
            run_collect(login, Some(Path::new(&program_arguments[3])))
        }
        [Some("render"), Some(snapshot_path), Some("--output-dir"), _]
            if !snapshot_path.starts_with('-') =>
        {
            run_render(
                Path::new(snapshot_path),
                Path::new(&program_arguments[3]),
                DragonBody::default(),
            )
        }
        [
            Some("render"),
            Some(snapshot_path),
            Some("--output-dir"),
            _,
            Some("--body"),
            Some(body_name),
        ] if !snapshot_path.starts_with('-') && parse_body(body_name).is_some() => run_render(
            Path::new(snapshot_path),
            Path::new(&program_arguments[3]),
            parse_body(body_name).expect("the guard parsed the body"),
        ),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run_collect(login: &str, output_path: Option<&Path>) -> ExitCode {
    let collection_result = collect_account_snapshot(&GhCli, login, current_utc_date())
        .map_err(|collection_error| collection_error.to_string())
        .and_then(|account_snapshot| {
            serde_json::to_string_pretty(&account_snapshot)
                .map_err(|serialize_error| serialize_error.to_string())
        })
        .and_then(|snapshot_json| write_snapshot(&format!("{snapshot_json}\n"), output_path));
    match collection_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("git-dragon: {error_message}");
            ExitCode::FAILURE
        }
    }
}

fn run_render(snapshot_path: &Path, output_directory: &Path, dragon_body: DragonBody) -> ExitCode {
    let render_result = render_day(snapshot_path, dragon_body).and_then(|rendered_day| {
        fs::create_dir_all(output_directory)
            .map_err(|create_error| format!("{}: {create_error}", output_directory.display()))?;
        let mut written_paths = Vec::new();
        for (file_name, file_bytes) in rendered_day.output_files {
            let output_path = output_directory.join(file_name);
            replace_file(&output_path, &file_bytes)?;
            written_paths.push(output_path.display().to_string());
        }
        println!(
            "{}: wrote {}",
            rendered_day.caption,
            written_paths.join(", ")
        );
        Ok(())
    });
    match render_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("git-dragon: {error_message}");
            ExitCode::FAILURE
        }
    }
}

/// The day's files, by name, with a caption naming the behavior, observation date, and body.
struct RenderedDay {
    caption: String,
    output_files: [(&'static str, Vec<u8>); 3],
}

/// Render and encode the day's files in memory, so none is written unless all succeed.
fn render_day(snapshot_path: &Path, dragon_body: DragonBody) -> Result<RenderedDay, String> {
    let snapshot_json = fs::read_to_string(snapshot_path)
        .map_err(|read_error| format!("{}: {read_error}", snapshot_path.display()))?;
    let account_snapshot: AccountSnapshot = serde_json::from_str(&snapshot_json)
        .map_err(|parse_error| format!("{}: {parse_error}", snapshot_path.display()))?;
    let dragon_development = derive_dragon_development(&account_snapshot)
        .map_err(|development_error| development_error.to_string())?;
    let behavior = select_behavior(&dragon_development);
    let flight_frames: Vec<DragonFrame> = (0..frame_count(behavior))
        .map(|frame_index| render_flight_frame(&dragon_development, dragon_body, behavior, frame_index))
        .collect();
    let gif_bytes =
        encode_gif(&flight_frames).map_err(|encoding_error| encoding_error.to_string())?;
    let png_bytes = encode_png(&render_dragon(&dragon_development, dragon_body))
        .map_err(|encoding_error| encoding_error.to_string())?;
    let caption = format!(
        "{} for {}, {} body",
        describe_behavior(behavior),
        dragon_development.observed_on,
        name_body(dragon_body)
    );
    let preview_html = encode_preview_html(&caption);
    Ok(RenderedDay {
        caption,
        output_files: [
            ("dragon.gif", gif_bytes),
            ("dragon.png", png_bytes),
            ("preview.html", preview_html.into_bytes()),
        ],
    })
}

fn parse_body(body_name: &str) -> Option<DragonBody> {
    BODY_NAMES
        .iter()
        .find(|(named_body_name, _)| *named_body_name == body_name)
        .map(|(_, dragon_body)| *dragon_body)
}

fn name_body(dragon_body: DragonBody) -> &'static str {
    BODY_NAMES
        .iter()
        .find(|(_, named_body)| *named_body == dragon_body)
        .map(|(body_name, _)| *body_name)
        .expect("every body has a name")
}

fn describe_behavior(behavior: Behavior) -> &'static str {
    match behavior {
        Behavior::DeepRest => "deep rest",
        Behavior::Rest => "rest",
        Behavior::SurfaceFeeding => "surface feeding",
        Behavior::Dart => "dart",
        Behavior::Cruise => "cruise",
    }
}

// GitHub dates its contribution calendar in UTC.
fn current_utc_date() -> NaiveDate {
    let seconds_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock is after 1970")
        .as_secs();
    DateTime::from_timestamp(
        i64::try_from(seconds_since_epoch).expect("the system clock is in range"),
        0,
    )
    .expect("the system clock is in range")
    .date_naive()
}

/// Write to standard output, or replace the file.
fn write_snapshot(snapshot_json: &str, output_path: Option<&Path>) -> Result<(), String> {
    let Some(output_path) = output_path else {
        return io::stdout()
            .write_all(snapshot_json.as_bytes())
            .map_err(|write_error| format!("standard output: {write_error}"));
    };
    replace_file(output_path, snapshot_json.as_bytes())
}

/// Replace the file through a sibling temporary file named after it, so a failed write
/// never leaves a partial file in place.
fn replace_file(output_path: &Path, file_bytes: &[u8]) -> Result<(), String> {
    let mut temporary_name = output_path.file_name().unwrap_or_default().to_owned();
    temporary_name.push(".tmp");
    let temporary_path = output_path.with_file_name(temporary_name);
    fs::write(&temporary_path, file_bytes)
        .and_then(|()| fs::rename(&temporary_path, output_path))
        .map_err(|write_error| {
            let _ = fs::remove_file(&temporary_path);
            format!("{}: {write_error}", output_path.display())
        })
}
