use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const PLUGIN_NAME: &str = "valkyrie";

const PLUGIN_INDEX_JS: &str = include_str!("../plugin/index.js");
const PLUGIN_PACKAGE_JSON: &str = include_str!("../plugin/package.json");

fn opencode_config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("Could not find config directory")
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
            || p.as_array().map(|arr| arr.first().map(|v| v.as_str() == Some(&plugin_path)).unwrap_or(false)).unwrap_or(false)
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
                && p.as_array().map(|arr| arr.first().map(|v| v.as_str() != Some(&plugin_path)).unwrap_or(true)).unwrap_or(true)
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
                || p.as_array().map(|arr| arr.first().map(|v| v.as_str() == Some(&plugin_path)).unwrap_or(false)).unwrap_or(false)
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
