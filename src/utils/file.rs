use gpui::Image;
use std::fs::File;

pub async fn write_image_to_temp_file(image: &Image) -> anyhow::Result<String> {
    let image_bytes = image.bytes();

    // Decode the image from bytes
    let img = image::load_from_memory(image_bytes)?;

    // Maximum dimensions for compression (adjust as needed)
    const MAX_WIDTH: u32 = 1560;
    const MAX_HEIGHT: u32 = 882;

    // Resize if image is too large
    let img = if img.width() > MAX_WIDTH || img.height() > MAX_HEIGHT {
        img.resize(MAX_WIDTH, MAX_HEIGHT, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Check if the image has an alpha channel
    let has_alpha = matches!(
        img.color(),
        image::ColorType::Rgba8
            | image::ColorType::Rgba16
            | image::ColorType::La8
            | image::ColorType::La16
    );

    if has_alpha {
        // For images with transparency, save as PNG to preserve alpha channel
        let temp_file =
            std::env::temp_dir().join(format!("{}.png", crate::utils::time::now_millis()));
        img.save_with_format(&temp_file, image::ImageFormat::Png)?;
        Ok(temp_file.to_string_lossy().to_string())
    } else {
        // For images without transparency, convert to JPEG for better compression
        let temp_file =
            std::env::temp_dir().join(format!("{}.jpg", crate::utils::time::now_millis()));

        // Convert to RGB if needed
        let rgb_img = img.to_rgb8();

        let mut file = File::create(&temp_file)?;
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 85);
        encoder.encode(
            &rgb_img,
            rgb_img.width(),
            rgb_img.height(),
            image::ExtendedColorType::Rgb8,
        )?;

        Ok(temp_file.to_string_lossy().to_string())
    }
}
