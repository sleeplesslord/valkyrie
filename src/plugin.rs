use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const PLUGIN_NAME: &str = "valkyrie";

const PLUGIN_INDEX_JS: &str = include_str!("../plugin/index.js");
const PLUGIN_PACKAGE_JSON: &str = include_str!("../plugin/package.json");

const CODEX_HOOK_SCRIPT: &str = include_str!("../codex/hooks/valkyrie_hook.py");
const CODEX_HOOKS_JSON_TEMPLATE: &str = include_str!("../codex/hooks.json");

// ── opencode ────────────────────────────────────────────────────────────

fn opencode_config_dir() -> PathBuf {
    // Always use $HOME/.config/opencode — dirs::config_dir() resolves to
    // ~/Library/Application Support on macOS which opencode doesn't use.
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".config")
        .join("opencode")
}

fn opencode_config_file() -> PathBuf {
    opencode_config_dir().join("opencode.json")
}

fn plugin_install_dir() -> PathBuf {
    opencode_config_dir().join("plugins").join(PLUGIN_NAME)
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct OpenCodeConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    plugin: Vec<serde_json::Value>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

pub fn install(force: bool) -> Result<()> {
    let plugin_dir = plugin_install_dir();

    if plugin_dir.exists() && !force {
        anyhow::bail!("Plugin already installed. Use --force to reinstall.");
    }

    fs::create_dir_all(&plugin_dir)?;

    let mut index_file = fs::File::create(plugin_dir.join("index.js"))?;
    index_file.write_all(PLUGIN_INDEX_JS.as_bytes())?;

    let mut package_file = fs::File::create(plugin_dir.join("package.json"))?;
    package_file.write_all(PLUGIN_PACKAGE_JSON.as_bytes())?;

    register_plugin()?;

    Ok(())
}

fn register_plugin() -> Result<()> {
    let config_file = opencode_config_file();

    let mut config: OpenCodeConfig = if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        OpenCodeConfig::default()
    };

    let plugin_path = format!("./plugins/{}", PLUGIN_NAME);
    let plugin_entry = serde_json::json!(plugin_path);

    let already_registered = config.plugin.iter().any(|p| {
        p.as_str() == Some(&plugin_path)
            || p.as_array()
                .map(|arr| {
                    arr.first()
                        .map(|v| v.as_str() == Some(&plugin_path))
                        .unwrap_or(false)
                })
                .unwrap_or(false)
    });

    if !already_registered {
        config.plugin.push(plugin_entry);

        fs::create_dir_all(config_file.parent().unwrap())?;
        let mut file = fs::File::create(&config_file)?;
        file.write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;
    }

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let plugin_dir = plugin_install_dir();
    let config_file = opencode_config_file();

    if plugin_dir.exists() {
        fs::remove_dir_all(&plugin_dir)?;
    }

    if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
        let mut config: OpenCodeConfig = serde_json::from_str(&content).unwrap_or_default();

        let plugin_path = format!("./plugins/{}", PLUGIN_NAME);
        config.plugin.retain(|p| {
            p.as_str() != Some(&plugin_path)
                && p.as_array()
                    .map(|arr| {
                        arr.first()
                            .map(|v| v.as_str() != Some(&plugin_path))
                            .unwrap_or(true)
                    })
                    .unwrap_or(true)
        });

        let mut file = fs::File::create(&config_file)?;
        file.write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;
    }

    Ok(())
}

pub fn status() -> Result<()> {
    let plugin_dir = plugin_install_dir();
    let config_file = opencode_config_file();

    let installed = plugin_dir.join("index.js").exists();
    let registered = if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
        let config: OpenCodeConfig = serde_json::from_str(&content).unwrap_or_default();
        let plugin_path = format!("./plugins/{}", PLUGIN_NAME);
        config.plugin.iter().any(|p| {
            p.as_str() == Some(&plugin_path)
                || p.as_array()
                    .map(|arr| {
                        arr.first()
                            .map(|v| v.as_str() == Some(&plugin_path))
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
        })
    } else {
        false
    };

    match (installed, registered) {
        (true, true) => {
            println!("Status: installed and registered");
            println!("Plugin dir: {}", plugin_dir.display());
        }
        (true, false) => {
            println!("Status: installed but NOT registered in opencode.json");
            println!("Run 'valkyrie install' to register.");
        }
        (false, _) => {
            println!("Status: not installed");
        }
    }

    Ok(())
}

// ── codex ───────────────────────────────────────────────────────────────

fn codex_config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".codex")
}

fn codex_hooks_dir() -> PathBuf {
    codex_config_dir().join("hooks")
}

fn codex_hooks_file() -> PathBuf {
    codex_config_dir().join("hooks.json")
}

fn valkyrie_state_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".valkyrie")
        .join("codex-state")
}

const HOOK_SCRIPT_NAME: &str = "valkyrie_hook.py";

/// Marker string in hook commands that identifies valkyrie hooks.
/// We search for this in existing hooks.json entries to detect our hooks.
fn is_valkyrie_hook(command: &str) -> bool {
    command.contains(HOOK_SCRIPT_NAME)
}

/// Generate hooks.json content with the placeholder replaced by the actual
/// absolute path to the installed hook script.
fn generate_hooks_json(hook_path: &str) -> String {
    CODEX_HOOKS_JSON_TEMPLATE.replace("__VALKYRIE_HOOK_PATH__", hook_path)
}

/// Recursively remove valkyrie hooks from a hooks JSON value (any structure).
/// Returns the cleaned value and a count of removed hooks.
fn remove_valkyrie_hooks(val: &Value) -> (Value, usize) {
    match val {
        Value::Object(obj) => {
            let mut cleaned = serde_json::Map::new();
            let mut removed = 0;
            for (k, v) in obj {
                // k is an event name like "SessionStart", "PreToolUse", etc.
                if let Value::Array(arr) = v {
                    let mut new_arr = Vec::new();
                    for matcher_group in arr {
                        match matcher_group {
                            Value::Object(group) => {
                                let hooks = group.get("hooks");
                                let _matcher = group.get("matcher");
                                if let Some(Value::Array(hooks_arr)) = hooks {
                                    let mut filtered = Vec::new();
                                    for hook in hooks_arr {
                                        if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                                            if is_valkyrie_hook(cmd) {
                                                removed += 1;
                                                continue;
                                            }
                                        }
                                        filtered.push(hook.clone());
                                    }
                                    if filtered.is_empty() {
                                        // Entire matcher group removed
                                        continue;
                                    } else {
                                        let mut new_group = group.clone();
                                        new_group.insert("hooks".into(), Value::Array(filtered));
                                        new_arr.push(Value::Object(new_group));
                                    }
                                } else {
                                    // No "hooks" key in this matcher group — keep as-is
                                    new_arr.push(matcher_group.clone());
                                }
                            }
                            _ => new_arr.push(matcher_group.clone()),
                        }
                    }
                    if !new_arr.is_empty() {
                        cleaned.insert(k.clone(), Value::Array(new_arr));
                    }
                } else {
                    // Non-array value (shouldn't happen in valid hooks.json)
                    let (cleaned_v, r) = remove_valkyrie_hooks(v);
                    removed += r;
                    cleaned.insert(k.clone(), cleaned_v);
                }
            }
            (Value::Object(cleaned), removed)
        }
        Value::Array(arr) => {
            let mut new_arr = Vec::new();
            let mut removed = 0;
            for item in arr {
                let (cleaned, r) = remove_valkyrie_hooks(item);
                removed += r;
                // Keep all items (we don't remove array elements at this level;
                // removal happens inside matcher groups)
                new_arr.push(cleaned);
            }
            (Value::Array(new_arr), removed)
        }
        _ => (val.clone(), 0),
    }
}

pub fn install_codex(force: bool) -> Result<()> {
    let hooks_dir = codex_hooks_dir();
    let hook_script_path = hooks_dir.join(HOOK_SCRIPT_NAME);

    if hook_script_path.exists() && !force {
        anyhow::bail!("Codex hooks already installed. Use --force to reinstall.");
    }

    // 1. Create ~/.codex/hooks/ directory
    fs::create_dir_all(&hooks_dir)?;

    // 2. Copy hook script
    let mut script_file = fs::File::create(&hook_script_path)?;
    script_file.write_all(CODEX_HOOK_SCRIPT.as_bytes())?;
    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_script_path, perms)?;
    }

    // 3. Merge hook definitions into ~/.codex/hooks.json
    let hook_cmd = format!("python3 {}", hook_script_path.to_string_lossy());
    let our_hooks_json = generate_hooks_json(&hook_cmd);

    let hooks_file = codex_hooks_file();
    let existing = if hooks_file.exists() {
        fs::read_to_string(&hooks_file)?
    } else {
        r#"{"hooks":{}}"#.to_string()
    };

    // Parse existing hooks.json
    let existing_val: Value = serde_json::from_str(&existing).unwrap_or_else(|_| {
        serde_json::json!({"hooks": {}})
    });

    // Remove previous valkyrie hooks (if any)
    let (cleaned, _removed) = remove_valkyrie_hooks(&existing_val);

    // Parse our hooks template (with real path)
    let our_val: Value = serde_json::from_str(&our_hooks_json)?;

    // Merge our hooks into the cleaned existing hooks
    let merged = merge_hooks(&cleaned, &our_val);

    fs::create_dir_all(hooks_file.parent().unwrap())?;
    let mut file = fs::File::create(&hooks_file)?;
    file.write_all(serde_json::to_string_pretty(&merged)?.as_bytes())?;

    println!("Codex hooks installed!");
    println!("Hook script: {}", hook_script_path.display());
    println!("Hooks config: {}", hooks_file.display());
    println!();
    println!("Next steps:");
    println!("1. Restart Codex (or start a new session)");
    println!("2. Run /hooks in Codex to review and trust the valkyrie hooks");
    println!("3. Start valkyrie TUI in a tmux sidebar");

    Ok(())
}

/// Merge hooks from `src` into `dst`. Both are {"hooks": {event: [matcher_groups]}}.
/// For each event in src, append src's matcher groups to dst's array.
fn merge_hooks(dst: &Value, src: &Value) -> Value {
    let mut result = dst.clone();
    let dst_hooks = result
        .as_object_mut()
        .and_then(|o| o.get_mut("hooks"))
        .and_then(|h| h.as_object_mut());

    let src_hooks = src.get("hooks").and_then(|h| h.as_object());

    if let (Some(dst_h), Some(src_h)) = (dst_hooks, src_hooks) {
        for (event, src_arr) in src_h {
            if let Some(dst_arr) = dst_h.get_mut(event).and_then(|e| e.as_array_mut()) {
                // Append all src matcher groups to dst's array
                if let Some(src_groups) = src_arr.as_array() {
                    dst_arr.extend(src_groups.iter().cloned());
                }
            } else {
                // Event doesn't exist in dst — insert it
                dst_h.insert(event.clone(), src_arr.clone());
            }
        }
    }

    result
}

pub fn uninstall_codex() -> Result<()> {
    let hooks_dir = codex_hooks_dir();
    let hook_script_path = hooks_dir.join(HOOK_SCRIPT_NAME);
    let hooks_file = codex_hooks_file();

    // 1. Remove hook script
    if hook_script_path.exists() {
        fs::remove_file(&hook_script_path)?;
    }

    // 2. Remove valkyrie hooks from hooks.json
    if hooks_file.exists() {
        let content = fs::read_to_string(&hooks_file)?;
        let existing: Value = serde_json::from_str(&content).unwrap_or_else(|_| {
            serde_json::json!({"hooks": {}})
        });
        let (cleaned, removed) = remove_valkyrie_hooks(&existing);

        if removed > 0 {
            let mut file = fs::File::create(&hooks_file)?;
            file.write_all(serde_json::to_string_pretty(&cleaned)?.as_bytes())?;
        }
    }

    // 3. Clean up state files
    let state_dir = valkyrie_state_dir();
    if state_dir.exists() {
        let _ = fs::remove_dir_all(&state_dir);
    }

    println!("Codex hooks uninstalled!");
    if hooks_file.exists() {
        println!("Hooks config cleaned: {}", hooks_file.display());
    }

    Ok(())
}

pub fn status_codex() -> Result<()> {
    let hooks_dir = codex_hooks_dir();
    let hook_script_path = hooks_dir.join(HOOK_SCRIPT_NAME);
    let hooks_file = codex_hooks_file();

    let installed = hook_script_path.exists();

    let registered = if hooks_file.exists() {
        let content = fs::read_to_string(&hooks_file)?;
        let val: Value = serde_json::from_str(&content).unwrap_or_else(|_| {
            serde_json::json!({"hooks": {}})
        });
        count_valkyrie_hooks(&val) > 0
    } else {
        false
    };

    // Check for active codex signal files
    let signal_dir = dirs::home_dir()
        .expect("Could not find home directory")
        .join(".valkyrie")
        .join("agents");
    let active_sessions = if signal_dir.exists() {
        let mut count = 0;
        for entry in fs::read_dir(&signal_dir)? {
            if let Ok(e) = entry {
                if let Ok(content) = fs::read_to_string(e.path()) {
                    if let Ok(signal) = serde_json::from_str::<Value>(&content) {
                        if signal.get("agent_type").and_then(|t| t.as_str()) == Some("codex") {
                            count += 1;
                        }
                    }
                }
            }
        }
        count
    } else {
        0
    };

    match (installed, registered) {
        (true, true) => {
            println!("Status: installed and registered");
            println!("Hook script: {}", hook_script_path.display());
            println!("Hooks config: {}", hooks_file.display());
        }
        (true, false) => {
            println!("Status: script installed but hooks NOT in hooks.json");
            println!("Run 'valkyrie install --codex' to register.");
        }
        (false, _) => {
            println!("Status: not installed");
        }
    }

    if active_sessions > 0 {
        println!("Active codex sessions: {}", active_sessions);
    }

    Ok(())
}

/// Count how many valkyrie hooks exist in a hooks JSON value.
fn count_valkyrie_hooks(val: &Value) -> usize {
    let (_, removed) = remove_valkyrie_hooks(val);
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_valkyrie_hooks_from_empty() {
        let input = serde_json::json!({"hooks": {}});
        let (cleaned, removed) = remove_valkyrie_hooks(&input);
        assert_eq!(removed, 0);
        assert_eq!(cleaned, input);
    }

    #[test]
    fn remove_valkyrie_hooks_preserves_others() {
        let input = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            {"type": "command", "command": "python3 /other/hook.py"}
                        ]
                    }
                ]
            }
        });
        let (cleaned, removed) = remove_valkyrie_hooks(&input);
        assert_eq!(removed, 0);
        assert_eq!(cleaned, input);
    }

    #[test]
    fn remove_valkyrie_hooks_removes_ours() {
        let input = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "*",
                        "hooks": [
                            {"type": "command", "command": "python3 /home/user/.codex/hooks/valkyrie_hook.py"}
                        ]
                    }
                ],
                "Stop": [
                    {
                        "hooks": [
                            {"type": "command", "command": "python3 /home/user/.codex/hooks/valkyrie_hook.py"}
                        ]
                    }
                ]
            }
        });
        let (cleaned, removed) = remove_valkyrie_hooks(&input);
        assert_eq!(removed, 2);
        // PreToolUse array should be empty (removed), so the event key is gone
        assert!(cleaned
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .is_none());
        assert!(cleaned.get("hooks").and_then(|h| h.get("Stop")).is_none());
    }

    #[test]
    fn remove_valkyrie_hooks_keeps_non_valkyrie_in_same_event() {
        let input = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "*",
                        "hooks": [
                            {"type": "command", "command": "python3 /other/hook.py"},
                            {"type": "command", "command": "python3 /home/user/.codex/hooks/valkyrie_hook.py"}
                        ]
                    }
                ]
            }
        });
        let (cleaned, removed) = remove_valkyrie_hooks(&input);
        assert_eq!(removed, 1);
        let pre = cleaned
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .and_then(|e| e.as_array())
            .unwrap();
        assert_eq!(pre.len(), 1);
        let hooks = pre[0].get("hooks").and_then(|h| h.as_array()).unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(
            hooks[0].get("command").and_then(|c| c.as_str()),
            Some("python3 /other/hook.py")
        );
    }

    #[test]
    fn merge_hooks_combines_events() {
        let dst = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {"matcher": "Bash", "hooks": [{"type": "command", "command": "other.py"}]}
                ]
            }
        });
        let src = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {"matcher": "*", "hooks": [{"type": "command", "command": "valkyrie_hook.py"}]}
                ],
                "Stop": [
                    {"hooks": [{"type": "command", "command": "valkyrie_hook.py"}]}
                ]
            }
        });
        let merged = merge_hooks(&dst, &src);
        let pre = merged
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .and_then(|e| e.as_array())
            .unwrap();
        assert_eq!(pre.len(), 2); // both matcher groups
        let stop = merged
            .get("hooks")
            .and_then(|h| h.get("Stop"))
            .and_then(|e| e.as_array())
            .unwrap();
        assert_eq!(stop.len(), 1);
    }
}
