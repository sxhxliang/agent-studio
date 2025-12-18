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
            Icon::FolderSync => "icons2/folder-sync.svg",
            Icon::Monitor => "icons2/monitor.svg",
            Icon::Trash2 => "icons2/trash-2.svg",
            Icon::SquarePause => "icons2/square-pause.svg",
        }
        .into()
    }
}
