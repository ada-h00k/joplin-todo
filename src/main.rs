// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 joplin-todo contributors

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    error::Error,
    fmt,
    fs,
    path::PathBuf,
    process,
    time::Duration,
};

const FIRST_JOPLIN_PORT: u16 = 41184;
const LAST_JOPLIN_PORT: u16 = 41194;
const PING_RESPONSE: &str = "JoplinClipperServer";

#[derive(Debug)]
struct CliError(String);

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CliError {}

enum Command {
    Create { title: String },
    NotebookList,
    NotebookSet { selector: String },
    NotebookShow,
    NotebookClear,
    Help,
    Version,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_notebook_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_notebook_path: Option<String>,
}

#[derive(Serialize)]
struct NewTodo<'a> {
    title: &'a str,
    body: &'a str,
    is_todo: u8,

    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<&'a str>,
}

#[derive(Deserialize)]
struct CreatedNote {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PaginatedResponse<T> {
    items: Vec<T>,

    #[serde(default)]
    has_more: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ListResponse<T> {
    Paginated(PaginatedResponse<T>),
    Array(Vec<T>),
}

#[derive(Debug, Deserialize)]
struct Folder {
    id: String,
    title: String,

    #[serde(default)]
    children: Vec<Folder>,
}

#[derive(Debug)]
struct Notebook {
    id: String,
    title: String,
    path: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    match parse_command()? {
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Version => {
            println!("todo {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::NotebookShow => {
            show_default_notebook()?;
            Ok(())
        }
        Command::NotebookClear => {
            clear_default_notebook()?;
            Ok(())
        }
        Command::NotebookList => {
            let (client, port, token) = connect_to_joplin()?;
            let notebooks = fetch_notebooks(&client, port, &token)?;
            print_notebooks(&notebooks);
            Ok(())
        }
        Command::NotebookSet { selector } => {
            let (client, port, token) = connect_to_joplin()?;
            let notebooks = fetch_notebooks(&client, port, &token)?;
            let notebook = resolve_notebook(&selector, &notebooks)?;

            let config = Config {
                default_notebook_id: Some(notebook.id.clone()),
                default_notebook_path: Some(notebook.path.clone()),
            };

            save_config(&config)?;
            println!("Default notebook set to: {}", notebook.path);
            Ok(())
        }
        Command::Create { title } => {
            let (client, port, token) = connect_to_joplin()?;
            create_todo(&client, port, &token, &title)?;
            Ok(())
        }
    }
}

fn parse_command() -> Result<Command, CliError> {
    let arguments: Vec<String> = env::args().skip(1).collect();

    if arguments.is_empty() {
        return Err(CliError(
            "No task was provided. Run `todo --help` for usage information.".to_owned(),
        ));
    }

    match arguments[0].as_str() {
        "-h" | "--help" => {
            ensure_argument_count(&arguments, 1, "todo --help")?;
            Ok(Command::Help)
        }
        "-V" | "--version" => {
            ensure_argument_count(&arguments, 1, "todo --version")?;
            Ok(Command::Version)
        }
        "notebook" => parse_notebook_command(&arguments[1..]),
        _ => Ok(Command::Create {
            title: arguments.join(" "),
        }),
    }
}

fn parse_notebook_command(arguments: &[String]) -> Result<Command, CliError> {
    let Some(subcommand) = arguments.first() else {
        return Err(CliError(
            "Missing notebook command. Use `todo notebook list`, `set`, `show`, or `clear`."
                .to_owned(),
        ));
    };

    match subcommand.as_str() {
        "list" => {
            ensure_argument_count(arguments, 1, "todo notebook list")?;
            Ok(Command::NotebookList)
        }
        "show" => {
            ensure_argument_count(arguments, 1, "todo notebook show")?;
            Ok(Command::NotebookShow)
        }
        "clear" => {
            ensure_argument_count(arguments, 1, "todo notebook clear")?;
            Ok(Command::NotebookClear)
        }
        "set" => {
            if arguments.len() < 2 {
                return Err(CliError(
                    "No notebook was provided. Example: todo notebook set \"Work/Tasks\""
                        .to_owned(),
                ));
            }

            let selector = arguments[1..].join(" ");
            if selector.trim().is_empty() {
                return Err(CliError("The notebook name must not be empty.".to_owned()));
            }

            Ok(Command::NotebookSet { selector })
        }
        _ => Err(CliError(format!(
            "Unknown notebook command `{subcommand}`. Use `list`, `set`, `show`, or `clear`."
        ))),
    }
}

fn ensure_argument_count(
    arguments: &[String],
    expected: usize,
    usage: &str,
) -> Result<(), CliError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(CliError(format!("Unexpected arguments. Usage: {usage}")))
    }
}

fn print_help() {
    println!(
        "todo {version}\n\nCreate a to-do note in the running Joplin Desktop application.\n\nUSAGE:\n    todo <TASK>\n    todo notebook list\n    todo notebook set <NAME_OR_PATH>\n    todo notebook show\n    todo notebook clear\n\nEXAMPLES:\n    todo Buy milk\n    todo \"Call Alex about the proposal\"\n    todo notebook list\n    todo notebook set \"Work/Tasks\"\n\nDEFAULT NOTEBOOK:\n    `notebook set` stores a Joplin notebook ID in the local configuration.\n    Nested notebooks can be selected with their displayed path.\n    Use `notebook clear` to return to Joplin's default placement.\n\nTOKEN:\n    Set JOPLIN_TOKEN, or store the token in:\n    $XDG_CONFIG_HOME/joplin-todo/token\n    ~/.config/joplin-todo/token\n",
        version = env!("CARGO_PKG_VERSION")
    );
}

fn connect_to_joplin() -> Result<(Client, u16, String), Box<dyn Error>> {
    let token = read_token()?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(10))
        .build()?;

    let port = find_joplin_port(&client).ok_or_else(|| {
        CliError(
            "Could not find a running Joplin Data API. Start Joplin Desktop and enable the Web Clipper service under Tools > Options > Web Clipper."
                .to_owned(),
        )
    })?;

    Ok((client, port, token))
}

fn create_todo(
    client: &Client,
    port: u16,
    token: &str,
    title: &str,
) -> Result<(), Box<dyn Error>> {
    let config = load_config()?;
    let payload = NewTodo {
        title,
        body: "",
        is_todo: 1,
        parent_id: config.default_notebook_id.as_deref(),
    };

    let response = client
        .post(format!("http://127.0.0.1:{port}/notes"))
        .query(&[("token", token)])
        .json(&payload)
        .send()?;

    let status = response.status();
    let response_body = response.text()?;

    if !status.is_success() {
        let notebook_hint = config
            .default_notebook_path
            .as_deref()
            .map(|path| {
                format!(
                    " The configured notebook `{path}` may have been deleted; run `todo notebook set` again or `todo notebook clear`."
                )
            })
            .unwrap_or_default();

        return Err(CliError(format!(
            "Joplin returned HTTP {status}: {response_body}.{notebook_hint}"
        ))
        .into());
    }

    let note_id = serde_json::from_str::<CreatedNote>(&response_body)
        .ok()
        .and_then(|note| note.id);

    println!("Created Joplin to-do: {title}");

    if let Some(path) = config.default_notebook_path {
        println!("Notebook: {path}");
    }

    if let Some(note_id) = note_id {
        println!("Note ID: {note_id}");
    }

    Ok(())
}

fn fetch_notebooks(
    client: &Client,
    port: u16,
    token: &str,
) -> Result<Vec<Notebook>, Box<dyn Error>> {
    let mut folders = Vec::new();
    let mut page = 1_u32;

    loop {
        let response = client
            .get(format!("http://127.0.0.1:{port}/folders"))
            .query(&[
                ("token", token.to_owned()),
                ("page", page.to_string()),
                ("limit", "100".to_owned()),
            ])
            .send()?;

        let status = response.status();
        let response_body = response.text()?;

        if !status.is_success() {
            return Err(CliError(format!(
                "Joplin returned HTTP {status} while reading notebooks: {response_body}"
            ))
            .into());
        }

        let parsed: ListResponse<Folder> =
            serde_json::from_str(&response_body).map_err(|error| {
                CliError(format!(
                    "Could not parse the notebook list returned by Joplin: {error}"
                ))
            })?;

        match parsed {
            ListResponse::Array(items) => {
                folders.extend(items);
                break;
            }
            ListResponse::Paginated(result) => {
                folders.extend(result.items);

                if !result.has_more {
                    break;
                }
            }
        }

        page = page.checked_add(1).ok_or_else(|| {
            CliError("Too many notebook result pages returned by Joplin.".to_owned())
        })?;
    }

    let mut notebooks = Vec::new();
    let mut seen_ids = HashSet::new();
    flatten_folders(&folders, None, &mut notebooks, &mut seen_ids);
    notebooks.sort_by_key(|notebook| notebook.path.to_lowercase());

    Ok(notebooks)
}

fn flatten_folders(
    folders: &[Folder],
    parent_path: Option<&str>,
    output: &mut Vec<Notebook>,
    seen_ids: &mut HashSet<String>,
) {
    for folder in folders {
        let path = match parent_path {
            Some(parent) => format!("{parent}/{}", folder.title),
            None => folder.title.clone(),
        };

        if seen_ids.insert(folder.id.clone()) {
            output.push(Notebook {
                id: folder.id.clone(),
                title: folder.title.clone(),
                path: path.clone(),
            });
        }

        flatten_folders(&folder.children, Some(&path), output, seen_ids);
    }
}

fn print_notebooks(notebooks: &[Notebook]) {
    if notebooks.is_empty() {
        println!("Joplin did not return any notebooks.");
        return;
    }

    println!("Available notebooks:");
    for notebook in notebooks {
        println!("  {}", notebook.path);
    }
}

fn resolve_notebook<'a>(
    selector: &str,
    notebooks: &'a [Notebook],
) -> Result<&'a Notebook, CliError> {
    let selector = selector.trim();

    let exact_paths: Vec<&Notebook> = notebooks
        .iter()
        .filter(|notebook| notebook.path == selector)
        .collect();
    if let Some(notebook) = require_unique_match(selector, exact_paths)? {
        return Ok(notebook);
    }

    let case_insensitive_paths: Vec<&Notebook> = notebooks
        .iter()
        .filter(|notebook| notebook.path.eq_ignore_ascii_case(selector))
        .collect();
    if let Some(notebook) = require_unique_match(selector, case_insensitive_paths)? {
        return Ok(notebook);
    }

    let exact_titles: Vec<&Notebook> = notebooks
        .iter()
        .filter(|notebook| notebook.title == selector)
        .collect();
    if let Some(notebook) = require_unique_match(selector, exact_titles)? {
        return Ok(notebook);
    }

    let case_insensitive_titles: Vec<&Notebook> = notebooks
        .iter()
        .filter(|notebook| notebook.title.eq_ignore_ascii_case(selector))
        .collect();
    if let Some(notebook) = require_unique_match(selector, case_insensitive_titles)? {
        return Ok(notebook);
    }

    Err(CliError(format!(
        "No notebook matched `{selector}`. Run `todo notebook list` to see available notebooks."
    )))
}

fn require_unique_match<'a>(
    selector: &str,
    matches: Vec<&'a Notebook>,
) -> Result<Option<&'a Notebook>, CliError> {
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.into_iter().next()),
        _ => {
            let mut paths: Vec<&str> = matches
                .iter()
                .map(|notebook| notebook.path.as_str())
                .collect();
            paths.sort_unstable();
            paths.dedup();

            Err(CliError(format!(
                "Notebook `{selector}` is ambiguous. Use one of these full paths:\n  - {}",
                paths.join("\n  - ")
            )))
        }
    }
}

fn show_default_notebook() -> Result<(), CliError> {
    let config = load_config()?;

    match (config.default_notebook_path, config.default_notebook_id) {
        (Some(path), Some(id)) => {
            println!("Default notebook: {path}");
            println!("Notebook ID: {id}");
        }
        _ => println!("No default notebook is configured."),
    }

    Ok(())
}

fn clear_default_notebook() -> Result<(), CliError> {
    let path = config_file_path()?;

    if path.exists() {
        fs::remove_file(&path).map_err(|error| {
            CliError(format!(
                "Could not remove the configuration file {}: {error}",
                path.display()
            ))
        })?;
        println!("Default notebook configuration cleared.");
    } else {
        println!("No default notebook was configured.");
    }

    Ok(())
}

fn load_config() -> Result<Config, CliError> {
    let path = config_file_path()?;

    if !path.exists() {
        return Ok(Config::default());
    }

    let content = fs::read_to_string(&path).map_err(|error| {
        CliError(format!(
            "Could not read the configuration file {}: {error}",
            path.display()
        ))
    })?;

    serde_json::from_str(&content).map_err(|error| {
        CliError(format!(
            "Could not parse the configuration file {}: {error}",
            path.display()
        ))
    })
}

fn save_config(config: &Config) -> Result<(), CliError> {
    let path = config_file_path()?;
    let directory = path.parent().ok_or_else(|| {
        CliError("Could not determine the configuration directory.".to_owned())
    })?;

    fs::create_dir_all(directory).map_err(|error| {
        CliError(format!(
            "Could not create the configuration directory {}: {error}",
            directory.display()
        ))
    })?;

    let mut content = serde_json::to_string_pretty(config).map_err(|error| {
        CliError(format!("Could not serialize the configuration: {error}"))
    })?;
    content.push('\n');

    let temporary_path = path.with_extension(format!("json.tmp.{}", process::id()));
    fs::write(&temporary_path, content).map_err(|error| {
        CliError(format!(
            "Could not write the temporary configuration file {}: {error}",
            temporary_path.display()
        ))
    })?;

    set_private_permissions(&temporary_path)?;

    fs::rename(&temporary_path, &path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        CliError(format!(
            "Could not replace the configuration file {}: {error}",
            path.display()
        ))
    })?;

    Ok(())
}

#[cfg(unix)]
fn set_private_permissions(path: &PathBuf) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        CliError(format!(
            "Could not set permissions on {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &PathBuf) -> Result<(), CliError> {
    Ok(())
}

fn read_token() -> Result<String, CliError> {
    if let Ok(token) = env::var("JOPLIN_TOKEN") {
        let token = token.trim();

        if !token.is_empty() {
            return Ok(token.to_owned());
        }
    }

    let token_path = token_file_path()?;
    let token = fs::read_to_string(&token_path).map_err(|error| {
        CliError(format!(
            "Could not read the Joplin token from {}: {error}. Create the file or set JOPLIN_TOKEN.",
            token_path.display()
        ))
    })?;

    let token = token.trim();

    if token.is_empty() {
        return Err(CliError(format!(
            "The token file {} is empty.",
            token_path.display()
        )));
    }

    Ok(token.to_owned())
}

fn application_config_directory() -> Result<PathBuf, CliError> {
    let config_directory = match env::var_os("XDG_CONFIG_HOME") {
        Some(directory) if !directory.is_empty() => PathBuf::from(directory),
        _ => {
            let home = env::var_os("HOME").ok_or_else(|| {
                CliError(
                    "Neither XDG_CONFIG_HOME nor HOME is set. Set JOPLIN_TOKEN instead."
                        .to_owned(),
                )
            })?;

            PathBuf::from(home).join(".config")
        }
    };

    Ok(config_directory.join("joplin-todo"))
}

fn token_file_path() -> Result<PathBuf, CliError> {
    Ok(application_config_directory()?.join("token"))
}

fn config_file_path() -> Result<PathBuf, CliError> {
    Ok(application_config_directory()?.join("config.json"))
}

fn find_joplin_port(client: &Client) -> Option<u16> {
    (FIRST_JOPLIN_PORT..=LAST_JOPLIN_PORT)
        .find(|port| is_joplin_port(client, *port))
}

fn is_joplin_port(client: &Client, port: u16) -> bool {
    let response = client
        .get(format!("http://127.0.0.1:{port}/ping"))
        .timeout(Duration::from_millis(400))
        .send();

    match response {
        Ok(response) if response.status().is_success() => response
            .text()
            .map(|body| body.trim() == PING_RESPONSE)
            .unwrap_or(false),
        _ => false,
    }
}
