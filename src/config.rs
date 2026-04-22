//! Configuration module for glzswitch.
//! Handles theme discovery, management, and swapping for GlazeWM configurations.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};

const GLAZEWM_CONFIG: &str = "config.yaml";
const TACKY_BORDERS_CONFIG: &str = "config.yaml";
const THEMES_DIR: &str = "themes";
const GLAZE_FILE: &str = "glaze.yaml";
const TACKY_BORDERS_FILE: &str = "tacky-borders.yaml";

/// Paths configuration for glzswitch themes and config files.
pub struct ConfigPaths {
    /// Directory containing theme files (~/.config/glzswitch/themes/)
    pub theme_dir: PathBuf,
    /// Path to the active GlazeWM config (~/.glzr/glazewm/config.yaml)
    pub glazewm_config: PathBuf,
    /// Tacky-borders config directory (~/.config/tacky-borders/)
    pub tacky_borders_dir: Option<PathBuf>,
    /// Tacky-borders config file path (~/.config/tacky-borders/config.yaml)
    pub tacky_borders_config: Option<PathBuf>,
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

        // Detect tacky-borders config directory (~/.config/tacky-borders/)
        let tacky_borders_dir = dirs::home_dir().map(|h| h.join(".config").join("tacky-borders"));

        let tacky_borders_config = tacky_borders_dir
            .as_ref()
            .map(|d| d.join(TACKY_BORDERS_CONFIG));

        Some(ConfigPaths {
            theme_dir,
            glazewm_config,
            tacky_borders_dir,
            tacky_borders_config,
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

    /// Check if tacky-borders config directory exists
    pub fn has_tacky_borders(&self) -> bool {
        self.tacky_borders_dir
            .as_ref()
            .map(|d| d.exists())
            .unwrap_or(false)
    }
}

/// Discover all theme subdirectory in the themes directory.
///
/// # Errors
/// Returns an error if the themes directory cannot be read.
pub fn get_themes(paths: &ConfigPaths) -> Result<Vec<PathBuf>> {
    tracing::debug!("Scanning theme directory: {}", paths.theme_dir.display());

    if !paths.theme_dir.exists() {
        return Ok(vec![]);
    }

    let mut themes: Vec<PathBuf> = fs::read_dir(&paths.theme_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();

    tracing::info!("Discovered {} themes", themes.len());
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

/// Create a default theme by copying the existing configs.
///
/// This should be called on first run when no themes exist.
/// Saves the user's current GlazeWM and tacky-borders configs to a "user-default" theme.
/// Returns true if a default theme was created, false otherwise.
///
/// # Errors
/// Returns an error if file operations fail.
pub fn create_default_theme_if_needed(paths: &ConfigPaths) -> Result<bool> {
    let themes = get_themes(paths)?;

    if themes.is_empty() {
        // Create user-default theme from existing configs
        let default_theme_dir = paths.theme_dir.join("user-default");

        // Ensure theme directory exists
        fs::create_dir_all(&default_theme_dir)?;

        let mut created_any = false;

        // Copy GlazeWM config if it exists
        let glazewm_source = paths.glazewm_config.clone();
        if glazewm_source.exists() {
            let target = default_theme_dir.join(GLAZE_FILE);
            fs::copy(&glazewm_source, &target)?;
            tracing::info!(
                "Created default theme from existing GlazeWM config: {}",
                target.display()
            );
            created_any = true;
        } else {
            tracing::warn!(
                "No existing GlazeWM config found at: {}",
                glazewm_source.display()
            );
        }

        // Copy tacky-borders config if directory exists
        if paths.has_tacky_borders() {
            if let Some(ref source) = paths.tacky_borders_config {
                if source.exists() {
                    let target = default_theme_dir.join(TACKY_BORDERS_FILE);
                    fs::copy(source, &target)?;
                    tracing::info!(
                        "Created default theme from existing tacky-borders config: {}",
                        target.display()
                    );
                    created_any = true;
                }
            }
        }

        return Ok(created_any);
    }

    Ok(false)
}

/// Swap the active configs with a theme.
///
/// Copies the source theme files to the target locations.
/// - glaze.yaml -> ~/.glzr/glazewm/config.yaml
/// - tacky-borders.yaml -> ~/.config/tacky-borders/config.yaml (if both exist)
///
/// # Arguments
/// * `theme_dir` - Path to the theme directory
/// * `paths` - Configuration paths containing target locations
///
/// # Errors
/// Returns an error if file operations fail.
fn copy_with_retry(source: &Path, target: &Path, max_retries: u32) -> Result<()> {
    for attempt in 0..max_retries {
        // Delete target first to ensure file watcher detects change
        if target.exists() {
            if let Err(e) = fs::remove_file(target) {
                if attempt < max_retries - 1 {
                    tracing::warn!("Failed to delete target, retrying: {}", e);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                return Err(e).context(format!("Failed to delete {}", target.display()));
            }
        }

        match fs::copy(source, target) {
            Ok(_) => return Ok(()),
            Err(e) if attempt < max_retries - 1 => {
                tracing::warn!(
                    "Copy failed (attempt {}/{}): {}",
                    attempt + 1,
                    max_retries,
                    e
                );
                std::thread::sleep(std::time::Duration::from_millis(
                    (100 * (attempt + 1)) as u64,
                ));
            }
            Err(e) => {
                return Err(e).context(format!(
                    "Failed to copy {} to {}",
                    source.display(),
                    target.display()
                ));
            }
        }
    }
    Ok(())
}

pub fn swap_theme(theme_dir: &Path, paths: &ConfigPaths) -> Result<()> {
    let glaze_source = theme_dir.join(GLAZE_FILE);
    if !glaze_source.exists() {
        anyhow::bail!("Theme GlazeWM config not found: {}", glaze_source.display());
    }

    copy_with_retry(&glaze_source, &paths.glazewm_config, 5)?;
    tracing::info!(
        "Applied GlazeWM config: {} -> {}",
        glaze_source.display(),
        paths.glazewm_config.display()
    );

    if paths.has_tacky_borders() {
        let tacky_source = theme_dir.join(TACKY_BORDERS_FILE);
        tracing::info!(
            "Tacky-borders source: {} (exists: {})",
            tacky_source.display(),
            tacky_source.exists()
        );
        if tacky_source.exists() {
            if let Some(ref target) = paths.tacky_borders_config {
                tracing::info!(
                    "Applying tacky-borders: {} -> {}",
                    tacky_source.display(),
                    target.display()
                );
                copy_with_retry(&tacky_source, target, 10)?;
                tracing::info!(
                    "Applied tacky-borders config: {} -> {}",
                    tacky_source.display(),
                    target.display()
                );
            }
        } else {
            tracing::info!(
                "No tacky-borders config in theme: {}",
                tacky_source.display()
            );
        }
    } else {
        if let Some(ref dir) = paths.tacky_borders_dir {
            tracing::info!("Tacky-borders dir does not exist: {}", dir.display());
        }
    }

    Ok(())
}

/// Apply a theme by name to both GlazeWM and tacky-borders.
///
/// Convenience function that resolves the theme name to a directory path.
///
/// # Arguments
/// * `paths` - Configuration paths
/// * `theme_name` - Name of the theme (directory name)
///
/// # Errors
/// Returns an error if the theme doesn't exist or swap fails.
pub fn apply_theme_by_name(paths: &ConfigPaths, theme_name: &str) -> Result<()> {
    // SECURITY: Validate theme_name to prevent path traversal
    if theme_name.contains("..") || theme_name.contains('/') || theme_name.contains('\\') {
        anyhow::bail!("Invalid theme name: contains path traversal characters");
    }

    let theme_dir = paths.theme_dir.join(theme_name);

    if !theme_dir.is_dir() {
        anyhow::bail!("Theme not found: {}", theme_name);
    }

    swap_theme(&theme_dir, paths)?;

    // Also save a copy in themes directory as "active" symlink or marker
    // For simplicity, just log the active theme
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
