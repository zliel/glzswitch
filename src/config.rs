//! Configuration module for glzswitch.
//! Handles theme discovery, management, and swapping for GlazeWM configurations.

use anyhow::Result;
use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};

const GLAZEWM_CONFIG: &str = "config.yaml";
const THEMES_DIR: &str = "themes";

/// Paths configuration for glzswitch themes and GlazeWM config.
pub struct ConfigPaths {
    /// Directory containing theme files (~/.config/glzswitch/themes/)
    pub theme_dir: PathBuf,
    /// Path to the active GlazeWM config (~/.glzr/glazewm/config.yaml)
    pub glazewm_config: PathBuf,
}

impl ConfigPaths {
    /// Create new ConfigPaths by detecting system directories.
    /// Returns None if system directories cannot be determined.
    pub fn new() -> Option<Self> {
        let project_dirs = ProjectDirs::from("com", "glzswitch", "glzswitch")?;

        let config_dir = project_dirs.config_dir();
        let theme_dir = config_dir.join(THEMES_DIR);

        // Primary: ~/.glzr/glazewm/config.yaml (GlazeWM spec location)
        // Fallback: ~/.config/glaze-wm/config.yaml (XDG standard)
        let glazewm_config = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".glzr")
            .join("glazewm")
            .join(GLAZEWM_CONFIG);

        Some(ConfigPaths {
            theme_dir,
            glazewm_config,
        })
    }

    /// Get the path to the GlazeWM config, checking multiple possible locations.
    /// Returns the first existing path.
    pub fn resolve_glazewm_config(&self) -> Option<PathBuf> {
        // Check the primary location (~/.glzr/glazewm/config.yaml)
        if self.glazewm_config.exists() {
            return Some(self.glazewm_config.clone());
        }

        // Fallback to XDG location (~/.config/glaze-wm/config.yaml)
        let xdg_config = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("glaze-wm")
            .join(GLAZEWM_CONFIG);

        if xdg_config.exists() {
            return Some(xdg_config);
        }

        None
    }
}

/// Discover all theme files (.yaml and .yml) in the themes directory.
///
/// # Errors
/// Returns an error if the themes directory cannot be read.
pub fn get_theme_files(paths: &ConfigPaths) -> Result<Vec<PathBuf>> {
    tracing::debug!("Scanning theme directory: {}", paths.theme_dir.display());

    let mut themes: Vec<PathBuf> = fs::read_dir(&paths.theme_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            let is_yaml = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false);

            tracing::debug!(
                "  {} - is_file: {}, is_yaml: {}",
                path.display(),
                path.is_file(),
                is_yaml
            );
            path.is_file() && is_yaml
        })
        .collect();

    tracing::info!("Discovered {} theme files", themes.len());
    for theme in &themes {
        tracing::info!(
            "  - {}",
            theme.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    themes.sort();
    Ok(themes)
}

/// Ensure the themes directory exists, creating it if necessary.
///
/// # Errors
/// Returns an error if directory creation fails.
pub fn ensure_themes_dir_exists(paths: &ConfigPaths) -> Result<()> {
    if !paths.theme_dir.exists() {
        fs::create_dir_all(&paths.theme_dir)?;
        tracing::info!("Created themes directory: {}", paths.theme_dir.display());
    }
    Ok(())
}

/// Create a default theme by copying the existing GlazeWM config.
///
/// This should be called on first run when no themes exist.
/// Returns true if a default theme was created, false otherwise.
///
/// # Errors
/// Returns an error if file operations fail.
pub fn create_default_theme_if_needed(paths: &ConfigPaths) -> Result<bool> {
    let themes = get_theme_files(paths)?;

    if themes.is_empty() {
        // Try to find an existing GlazeWM config
        let config_path = paths
            .resolve_glazewm_config()
            .unwrap_or_else(|| paths.glazewm_config.clone());

        if config_path.exists() {
            let default_theme = paths.theme_dir.join("default.yaml");
            fs::copy(&config_path, &default_theme)?;
            tracing::info!(
                "Created default theme from existing config: {}",
                default_theme.display()
            );
            return Ok(true);
        } else {
            tracing::warn!(
                "No existing GlazeWM config found at: {}",
                config_path.display()
            );
        }
    }

    Ok(false)
}

/// Swap the active GlazeWM config with a theme file.
///
/// Copies the source theme to the target location.
/// Ensures source exists before attempting the copy.
///
/// # Arguments
/// * `source` - Path to the new theme file
/// * `target` - Path where the theme should be applied (usually the GlazeWM config)
///
/// # Errors
/// Returns an error if file operations fail.
pub fn swap_theme(source: &Path, target: &Path) -> Result<()> {
    // First ensure source exists
    if !source.exists() {
        anyhow::bail!("Source theme not found: {}", source.display());
    }

    // Copy to target (overwrite)
    fs::copy(source, target)?;

    tracing::info!(
        "Applied theme: {} -> {}",
        source.display(),
        target.display()
    );
    Ok(())
}

/// Apply a theme by name to GlazeWM.
///
/// Convenience function that resolves the theme name to a file path.
///
/// # Arguments
/// * `paths` - Configuration paths
/// * `theme_name` - Name of the theme (without extension)
///
/// # Errors
/// Returns an error if the theme doesn't exist or swap fails.
pub fn apply_theme_by_name(paths: &ConfigPaths, theme_name: &str) -> Result<()> {
    // SECURITY: Validate theme_name to prevent path traversal
    if theme_name.contains("..") || theme_name.contains('/') || theme_name.contains('\\') {
        anyhow::bail!("Invalid theme name: contains path traversal characters");
    }

    let theme_file = paths.theme_dir.join(format!("{}.yaml", theme_name));

    if !theme_file.exists() {
        anyhow::bail!("Theme not found: {}", theme_name);
    }

    swap_theme(&theme_file, &paths.glazewm_config)?;

    // Also save a copy in themes directory as "active.yaml" for reference
    let active_theme = paths.theme_dir.join("active.yaml");
    let _ = fs::remove_file(&active_theme);
    fs::copy(&theme_file, &active_theme)?;

    tracing::info!("Applied theme: {}", theme_name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_paths_new() {
        let paths = ConfigPaths::new();
        assert!(paths.is_some());
        let paths = paths.unwrap();
        assert!(paths.theme_dir.to_string_lossy().contains("glzswitch"));
    }

    #[test]
    fn test_theme_dir_exists_check() {
        // Just verify the struct can be created
        let paths = ConfigPaths::new().unwrap();
        // Don't actually create directories in tests
        assert!(paths.theme_dir.to_string_lossy().contains("themes"));
    }
}
