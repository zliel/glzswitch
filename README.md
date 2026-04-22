# glzswitch

A Rust utility for switching [GlazeWM](https://github.com/glzr-io/glazewm) and [Tacky-Borders](https://github.com/lukeyou05/tacky-borders) themes via hotkey.

## Prerequisites

- Rust toolchain (1.56+ recommended)
- GlazeWM installed and running
- GlazeWM WebSocket server enabled (default port: 6123)
- Optional: Tacky-Borders for window border themes

## Installation

### Build from Source

1. Clone the repository:

   ```bash
   cd glzswitch
   ```

2. Build the application:

   ```bash
   cargo build --release --features console
   ```

   The `--features console` flag enables debug output to the terminal (Windows hides console output by default).

3. The binary will be available at `target/release/glzswitch.exe`

### Todo: Cargo Install

Currently this is only available to be built from source, but I plan to add a GitHub Actions workflow to automatically build and attach the latest release binary and make it available in releases and through cargo for easy installation.

## Theme Directory Structure

Themes are stored in subdirectories under `%appdata%/glzswitch/themes/<theme_name>/`:

```
themes/
├── theme-1/
│   ├── glaze.yaml          # GlazeWM config
│   └── Tacky-Borders.yaml # Optional Tacky-Borders config
├── theme-2/
│   ├── glaze.yaml
│   └── Tacky-Borders.yaml
└── user-default/       # Auto-created on first run
    ├── glaze.yaml
    └── Tacky-Borders.yaml
```

Each theme is a subdirectory containing:

- `glaze.yaml` - GlazeWM window manager configuration
- `Tacky-Borders.yaml` (optional) - Tacky-Borders window border configuration

## Usage

### CLI Commands

```bash
# Start the theme switcher daemon (runs in background, listens for hotkey)
glzswitch.exe

# Show help
glzswitch.exe -h

# Save current configs to the currently active theme
glzswitch.exe save

# Save current configs to a specific theme (creates if it doesn't exist)
glzswitch.exe save my-custom-theme

# Run in debug mode (see detailed logs)
cargo run --features console
```

### Configuration

By default, glzswitch looks for:

- Theme directories: `%appdata%/glzswitch/themes/`
  - Themes should be organized in subdirectories, each containing a `glaze.yaml` and optionally a `Tacky-Borders.yaml` (i.e. `my-theme/glaze.yaml` and `my-theme/Tacky-Borders.yaml`)
- GlazeWM config: `%userprofile%/.glzr/glazewm/config.yaml` (primary)
  - Fallback: `%userprofile%/.config/glaze-wm/config.yaml`
- GlazeWM WebSocket: `ws://localhost:6123`
- Tacky-borders config: `%userprofile%/.config/Tacky-Borders/config.yaml`

To start glzswitch with GlazeWM, add the following to your GlazeWM config file:

```yaml
startup_commands:
  - shell-exec zebar
  - shell-exec C:\path\to\glzswitch\target\release\glzswitch.exe
```

Replace the path with the actual location of your `glzswitch.exe` binary. Soon a GitHub Actions workflow will be added to automatically build and attach the latest release binary and make it available in releases and through cargo.

### Hotkey

The default hotkey is configured in `src/hotkey.rs` and can be modified there. Currently set to `Ctrl + Alt + Shift + Space` (my preferred hotkey for it from Omarchy).

Note: There's a 1500ms cooldown between hotkey presses to prevent issues with rapid theme switching. This is a known limitation, see "Known Issues" below.

## Note: How Theme Swapping Works

### Delete + Rename

glzswitch uses a specific approach to theme swapping that is necessary for Tacky-Borders to recognize configuration changes:

1. **First Run**: Creates a `user-default` theme with copies of your current GlazeWM and Tacky-Borders configs.

2. **Subsequent Swaps**:
   - Deletes the existing config file in the target config directory
   - Inserts a temporary file with the new theme content
   - Renames the temp file to `config.yaml`

The delete + rename pattern ensures the file watcher for Tacky-Borders detects the change.

### Preserving Your Customizations

If you make changes to your GlazeWM or Tacky-Borders configs and want to preserve them:

```bash
# Save current configs to the currently active theme
glzswitch.exe save

# Or save to a specific theme (creates it if it doesn't exist)
glzswitch.exe save my-custom-theme
```

This copies your live configs back to the theme template, so they'll be preserved on future theme switches.

Tip: To avoid having to use those commands or having your edits get overwritten, edit your theme files in `%appdata%/glzswitch/themes/<theme-name>/` rather than the live config directories.

## Troubleshooting

### Common Issues

1. **WebSocket Connection Failed**
   - Ensure GlazeWM is running with WebSocket server enabled
   - Check that no firewall is blocking GlazeWM's WebSocket port (default: 6123)
   - Verify the WebSocket address in `src/websocket.rs`

2. **Theme Not Found**
   - Verify theme directories exist in `%appdata%/glzswitch/themes/`
   - Ensure directories contain `glaze.yaml`
   - Check directory permissions

3. **Hotkey Not Working**
   - Conflicting hotkey with another application
   - Insufficient permissions for global hotkey registration
   - Check debug logs for hotkey registration messages

4. **OS Error 32 on Theme Swap**
   - Wait longer between theme switches
   - This is a known limitation

## Contributing

1. Fork the repository
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Inspired by and built for use with [GlazeWM](https://github.com/glzr-io/glazewm)
- Inspired by and built for use with [Tacky-Borders](https://github.com/lukeyou05/tacky-borders)

They've done incredible work and I really like the theme-switching process in Omarchy, so I want to help bring that experience to Windows for GlazeWM users.
