use anyhow::anyhow;
use gpui::*;
use gpui_component::IconNamed;
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "./assets"]
#[include = "icons/**/*.svg"]
#[include = "icons2/**/*.svg"]
#[include = "logo/**/*.svg"]
pub struct Assets;

#[derive(RustEmbed)]
#[folder = "./"]
#[include = "config.json"]
pub struct ConfigAssets;

#[derive(RustEmbed)]
#[folder = "./themes"]
#[include = "*.json"]
pub struct ThemeAssets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

pub enum Icon {
    Claude,
    Cursor,
    DeepSeek,
    Gemini,
    Kimi,
    MCP,
    Minimax,
    Moonshot,
    OpenAI,
    Qwen,
    Zai,
    Brain,
    ListTodo,
    CircleDashed,
    FolderSync,
    Monitor,
    Trash2,
    SquarePause,
}

impl IconNamed for Icon {
    fn path(self) -> SharedString {
        match self {
            Icon::Claude => "logo/claude.svg",
            Icon::Cursor => "logo/cursor.svg",
            Icon::DeepSeek => "logo/deepseek.svg",
            Icon::Gemini => "logo/gemini.svg",
            Icon::Kimi => "logo/kimi.svg",
            Icon::MCP => "logo/mcp.svg",
            Icon::Minimax => "logo/minimax.svg",
            Icon::Moonshot => "logo/moonshot.svg",
            Icon::OpenAI => "logo/openai.svg",
            Icon::Qwen => "logo/qwen.svg",
            Icon::Zai => "logo/zai.svg",
            Icon::Brain => "icons2/brain.svg",
            Icon::ListTodo => "icons2/list-todo.svg",
            Icon::CircleDashed => "icons2/circle-dashed.svg",
            Icon::FolderSync => "icons2/folder-sync.svg",
            Icon::Monitor => "icons2/monitor.svg",
            Icon::Trash2 => "icons2/trash-2.svg",
            Icon::SquarePause => "icons2/square-pause.svg",
        }
        .into()
    }
}

/// Get icon based on agent name
pub fn get_agent_icon(name: &str) -> Icon {
    let name_lower = name.to_lowercase();
    // TODO Check for specific agent names
    if name_lower.contains("claude") {
        crate::assets::Icon::Claude
    } else if name_lower.contains("cursor") {
        crate::assets::Icon::Cursor
    } else if name_lower.contains("deepseek") {
        crate::assets::Icon::DeepSeek
    } else if name_lower.contains("gemini") {
        crate::assets::Icon::Gemini
    } else if name_lower.contains("kimi") {
        crate::assets::Icon::Kimi
    } else if name_lower.contains("mcp") {
        crate::assets::Icon::MCP
    } else if name_lower.contains("minimax") {
        crate::assets::Icon::Minimax
    } else if name_lower.contains("moonshot") {
        crate::assets::Icon::Moonshot
    } else if name_lower.contains("openai") || name_lower.contains("codex") {
        crate::assets::Icon::OpenAI
    } else if name_lower.contains("qwen") || name_lower.contains("iflow") {
        crate::assets::Icon::Qwen
    } else if name_lower.contains("zai") {
        crate::assets::Icon::Zai
    } else {
        // Default to Claude icon if no match
        crate::assets::Icon::Claude
    }
}

/// Get default config.json content embedded in the binary
pub fn get_default_config() -> Option<String> {
    ConfigAssets::get("config.json").map(|file| {
        String::from_utf8_lossy(&file.data).to_string()
    })
}

/// Get all embedded theme files
pub fn get_embedded_themes() -> Vec<(String, String)> {
    ThemeAssets::iter()
        .filter_map(|name| {
            let name_str = name.to_string();
            if name_str.ends_with(".json") {
                ThemeAssets::get(&name_str).map(|file| {
                    let content = String::from_utf8_lossy(&file.data).to_string();
                    (name_str, content)
                })
            } else {
                None
            }
        })
        .collect()
}
