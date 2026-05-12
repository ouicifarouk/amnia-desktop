//! Safely append/remove an `amnia` alias in the user's shell rc file.
//!
//! The alias maps the user-chosen assistant name to a command that executes
//! the Python sidecar with `"$@"` as system arguments, e.g.:
//!
//! ```bash
//! # >>> amnia-desktop alias (managed) >>>
//! alias amnia='python3 /home/<user>/.local/share/amnia-desktop/python_sidecar/main.py "$@"'
//! # <<< amnia-desktop alias (managed) <<<
//! ```
//!
//! The block is delimited by sentinel comments so the file can be rewritten
//! idempotently without ever touching unrelated lines.

use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const BEGIN_MARK: &str = "# >>> amnia-desktop alias (managed) >>>";
const END_MARK: &str = "# <<< amnia-desktop alias (managed) <<<";

#[derive(Debug, Serialize)]
pub struct AliasResult {
    pub rc_file: String,
    pub alias_name: String,
    pub command: String,
}

#[tauri::command]
pub fn register_shell_alias(name: String) -> Result<AliasResult, String> {
    validate_alias_name(&name)?;

    let rc = pick_rc_file().ok_or_else(|| "Unable to locate a shell rc file".to_string())?;
    let sidecar_path = sidecar_install_path();
    let cmd = format!(
        "alias {name}='python3 {sidecar} \"$@\"'",
        name = name,
        sidecar = sidecar_path.display()
    );

    rewrite_managed_block(&rc, Some(&cmd)).map_err(|e| format!("rewrite failed: {e}"))?;

    Ok(AliasResult {
        rc_file: rc.display().to_string(),
        alias_name: name,
        command: cmd,
    })
}

#[tauri::command]
pub fn remove_shell_alias() -> Result<(), String> {
    let rc = pick_rc_file().ok_or_else(|| "Unable to locate a shell rc file".to_string())?;
    rewrite_managed_block(&rc, None).map_err(|e| format!("rewrite failed: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn validate_alias_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 32 {
        return Err("Alias name must be 1-32 characters".into());
    }
    let first = name.chars().next().unwrap();
    if !first.is_ascii_alphabetic() {
        return Err("Alias name must start with a letter".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Alias name may only contain letters, digits, '_' and '-'".into());
    }
    Ok(())
}

fn pick_rc_file() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let shell = std::env::var("SHELL").unwrap_or_default();
    let zshrc = home.join(".zshrc");
    let bashrc = home.join(".bashrc");

    if shell.ends_with("/zsh") && zshrc.exists() {
        return Some(zshrc);
    }
    if shell.ends_with("/bash") && bashrc.exists() {
        return Some(bashrc);
    }
    // Fall back to whichever exists, preferring bashrc on most Linux distros.
    if bashrc.exists() {
        Some(bashrc)
    } else if zshrc.exists() {
        Some(zshrc)
    } else {
        // Create ~/.bashrc rather than silently failing.
        Some(bashrc)
    }
}

fn sidecar_install_path() -> PathBuf {
    // When the .deb/AppImage is installed Tauri exposes a `resource_dir` to
    // the app, but the alias must be resolvable from *any* terminal — so we
    // standardise on the XDG data location written by the installer.
    let data = dirs::data_dir().unwrap_or_else(|| PathBuf::from("/usr/local/share"));
    data.join("amnia-desktop")
        .join("python_sidecar")
        .join("main.py")
}

/// Atomically rewrite the rc file, replacing the managed block. Pass `None`
/// to remove the block entirely.
fn rewrite_managed_block(rc: &Path, new_block: Option<&str>) -> std::io::Result<()> {
    let existing = fs::read_to_string(rc).unwrap_or_default();
    let mut out = String::with_capacity(existing.len() + 256);

    let mut inside = false;
    for line in existing.lines() {
        if line.trim() == BEGIN_MARK {
            inside = true;
            continue;
        }
        if inside {
            if line.trim() == END_MARK {
                inside = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    if let Some(cmd) = new_block {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(BEGIN_MARK);
        out.push('\n');
        out.push_str(cmd);
        out.push('\n');
        out.push_str(END_MARK);
        out.push('\n');
    }

    // Write to a sibling temp file then rename — never truncate in place.
    let tmp = rc.with_extension("amnia.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(out.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, rc)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn alias_validation() {
        assert!(validate_alias_name("amnia").is_ok());
        assert!(validate_alias_name("am-nia_2").is_ok());
        assert!(validate_alias_name("").is_err());
        assert!(validate_alias_name("2amnia").is_err());
        assert!(validate_alias_name("am nia").is_err());
        assert!(validate_alias_name(&"a".repeat(40)).is_err());
    }

    #[test]
    fn block_is_idempotent() {
        let dir = tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        fs::write(&rc, "export FOO=bar\n").unwrap();

        rewrite_managed_block(&rc, Some("alias amnia='echo hi'")).unwrap();
        rewrite_managed_block(&rc, Some("alias amnia='echo hello'")).unwrap();

        let after = fs::read_to_string(&rc).unwrap();
        assert!(after.contains("export FOO=bar"));
        assert_eq!(after.matches(BEGIN_MARK).count(), 1);
        assert!(after.contains("alias amnia='echo hello'"));
    }

    #[test]
    fn removing_block_leaves_other_content() {
        let dir = tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        fs::write(&rc, "export FOO=bar\n").unwrap();
        rewrite_managed_block(&rc, Some("alias amnia='echo hi'")).unwrap();
        rewrite_managed_block(&rc, None).unwrap();

        let after = fs::read_to_string(&rc).unwrap();
        assert_eq!(after.trim(), "export FOO=bar");
        assert!(!after.contains(BEGIN_MARK));
    }
}
