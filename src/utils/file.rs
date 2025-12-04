use gpui::Image;
use std::fs::File;
use std::io::Write;

pub async fn write_image_to_temp_file(image: &Image) -> anyhow::Result<String> {
    let image_bytes = image.bytes();
    let temp_file = std::env::temp_dir().join(format!("{}.png", crate::utils::time::now_millis()));

    // Write the image bytes to the file
    let mut file = File::create(&temp_file)?;
    file.write_all(image_bytes)?;

    Ok(temp_file.to_string_lossy().to_string())
}
