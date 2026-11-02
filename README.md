# joplin-todo

A small Rust command-line tool for Linux that creates to-do notes in the currently running Joplin Desktop application.

```
todo Buy milk
```

The complete command-line text becomes the title of a new, unfinished Joplin to-do note.

You can configure a default Joplin notebook once:

```
todo notebook set "Work/Tasks"
```

Every later to-do is then created in that notebook.

## How it works

The program connects to the local Joplin Data API exposed by Joplin Desktop's Web Clipper service. It automatically checks ports `41184` through `41194`, reads your API token, and sends a request that creates a note with `is_todo` set to `1`.

When a default notebook is configured, its Joplin folder ID is sent as the note's `parent_id`. Joplin internally calls notebooks “folders”.

The program only connects to `127.0.0.1`.

Joplin Data API documentation:

[https://joplinapp.org/help/api/references/rest_api/](https://joplinapp.org/help/api/references/rest_api/)

## Requirements

- Linux
- Joplin Desktop, running while the command is used
- Joplin Web Clipper service enabled
- Rust 1.70 or newer, including Cargo (only when building from source)

Install Rust with your distribution's package manager or with rustup.

## 1. Enable the Joplin Data API

In Joplin Desktop:

1. Open **Tools > Options > Web Clipper**.
2. Enable the Web Clipper service.
3. Copy the authorization token shown by Joplin.

Joplin Desktop must be running whenever you create a to-do, list notebooks, or select a notebook.

## 2. Store the token

The recommended location is:

```
~/.config/joplin-todo/token
```

Create it with:

```
mkdir -p ~/.config/joplin-todo
printf '%s\n' 'YOUR-JOPLIN-TOKEN' > ~/.config/joplin-todo/token
chmod 600 ~/.config/joplin-todo/token
```

When `XDG_CONFIG_HOME` is set, the program uses:

```
$XDG_CONFIG_HOME/joplin-todo/token
```

Alternatively, set an environment variable:

```
export JOPLIN_TOKEN='YOUR-JOPLIN-TOKEN'
```

`JOPLIN_TOKEN` takes precedence over the token file.

## 3. Install

### Option A: pre-built binary (no Rust needed)

Every tagged release on GitHub contains a statically linked binary for
`x86_64` and `aarch64`. It runs on any Linux distribution, including
immutable Fedora variants (Silverblue, Kinoite, Bazzite, ...), and does not
need Cargo, a toolbox or layered packages:

```
VERSION=v0.2.1
ARCH="$(uname -m)"
NAME="joplin-todo-$VERSION-$ARCH-linux"
BASE="https://github.com/ada-h00k/joplin-todo/releases/download/$VERSION"
curl -fLO "$BASE/$NAME.tar.gz"
curl -fLO "$BASE/$NAME.tar.gz.sha256"
sha256sum -c "$NAME.tar.gz.sha256"
tar xzf "$NAME.tar.gz"
install -Dm755 "$NAME/todo" ~/.local/bin/todo
```

`~/.local/bin` is in `PATH` by default on Fedora. To uninstall, run
`rm ~/.local/bin/todo`.

To publish a new release, bump `version` in `Cargo.toml`, commit, and push a
matching tag (`git tag v0.2.2 && git push origin v0.2.2`). The workflow in
`.github/workflows/release.yml` builds the binaries and attaches them to the
release.

### Option B: build from source

From the project directory:

```
cargo install --path .
```

Or use the included installer:

```
./install.sh
```

Cargo normally installs the executable as:

```
~/.cargo/bin/todo
```

Make sure that `~/.cargo/bin` is in your `PATH`. For Bash, add this to `~/.bashrc` when necessary:

```
export PATH="$HOME/.cargo/bin:$PATH"
```

Then reload the shell:

```
source ~/.bashrc
```

## Creating to-dos

```
todo Buy milk
todo Prepare the documents for Monday
todo "Call Alex: discuss the proposal"
```

## Configuring the default notebook

### List available notebooks

```
todo notebook list
```

Nested notebooks are displayed as paths:

```
Available notebooks:
  Personal
  Work
  Work/Projects
  Work/Tasks
```

### Select a default notebook

Use either a unique notebook name or its full path:

```
todo notebook set Tasks
todo notebook set "Work/Tasks"
```

Using the full path is recommended when multiple notebooks have the same name. Ambiguous names are rejected and the matching paths are displayed.

The selected notebook ID and its displayed path are stored in:

```
~/.config/joplin-todo/config.json
```

Or, when `XDG_CONFIG_HOME` is set:

```
$XDG_CONFIG_HOME/joplin-todo/config.json
```

The configuration file does not contain the Joplin API token.

### Show the current default

```
todo notebook show
```

Example:

```
Default notebook: Work/Tasks
Notebook ID: 0123456789abcdef0123456789abcdef
```

### Clear the default

```
todo notebook clear
```

After clearing the setting, the program no longer sends a `parent_id` when creating a to-do.

### Rename or delete a notebook

Joplin identifies the configured notebook by ID, so renaming a notebook does not normally stop to-dos from being created there. The path shown by `todo notebook show` is the path saved when the notebook was selected; run `todo notebook set` again to refresh it.

If the notebook is deleted, select another notebook or clear the setting:

```
todo notebook set "Another Notebook"
# or
todo notebook clear
```

## Other commands

Show help:

```
todo --help
```

Show the installed version:

```
todo --version
```

## Updating

Binary install: repeat the steps from Option A with the new version.

Source install: after changing or updating the source code, reinstall it with:

```
cargo install --path . --force
```

## Uninstalling

Binary install:

```
rm ~/.local/bin/todo
```

Source install:

```
cargo uninstall joplin-todo
```

The token and notebook configuration are not removed automatically. Remove them separately when no longer needed:

```
rm -rf ~/.config/joplin-todo
```

## Troubleshooting

### Invalid type: map, expected a sequence

This error was present in version `0.2.0`. Joplin returns collection endpoints as an object containing an `items` array. Upgrade to version `0.2.1` or newer and reinstall.

### Could not find a running Joplin Data API

Confirm that:

- Joplin Desktop is running.
- The Web Clipper service is enabled.
- A local firewall is not blocking connections to `127.0.0.1`.

### HTTP 403 or an authorization error

Copy the current token again from **Tools > Options > Web Clipper**, then update the token file or `JOPLIN_TOKEN` environment variable.

### No notebook matched

Run:

```
todo notebook list
```

Then copy the exact notebook name or full path into `todo notebook set`.

### Notebook name is ambiguous

Use the full path printed in the error message:

```
todo notebook set "Work/Tasks"
```

### Command not found

Check that the install directory is in your `PATH`:

```
echo "$PATH"
ls -l ~/.local/bin/todo ~/.cargo/bin/todo
```

## Security notes

- The token file should have permissions `600`.
- Do not commit the token to Git.
- Do not pass the token as a command-line argument, because command-line arguments can be visible to other processes.
- The notebook configuration contains a notebook ID and path, but no API token.

## License

This project is licensed under the GNU Affero General Public License, version 3 or later (`AGPL-3.0-or-later`). See [`LICENSE`](LICENSE) for the full license text.
