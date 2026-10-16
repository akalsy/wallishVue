use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, ImageReader};
use std::io::Cursor;

fn encode_jpeg(rgb: &image::RgbImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut out, quality);
    encoder
        .encode(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
        .map_err(|err| format!("图片无法保存（{err}）"))?;
    Ok(out)
}

pub fn open_image(bytes: &[u8]) -> Result<DynamicImage, String> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| format!("这张图片打不开（{err}）"))?
        .decode()
        .map_err(|_| "这张图片打不开".to_string())
}

pub fn limit_edge(image: DynamicImage) -> DynamicImage {
    let edge = image.width().max(image.height());
    if edge <= 4096 {
        return image;
    }
    let scale = 4096.0 / edge as f32;
    image.resize(
        ((image.width() as f32) * scale).max(1.0) as u32,
        ((image.height() as f32) * scale).max(1.0) as u32,
        image::imageops::FilterType::Lanczos3,
    )
}

pub fn preview_jpeg(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let edge = image.width().max(image.height());
    let preview = if edge <= 1280 {
        image.clone()
    } else {
        let scale = 1280.0 / edge as f32;
        image.resize(
            ((image.width() as f32) * scale).max(1.0) as u32,
            ((image.height() as f32) * scale).max(1.0) as u32,
            image::imageops::FilterType::Triangle,
        )
    };
    encode_jpeg(&preview.to_rgb8(), 82)
}

pub fn crop_cover(bytes: &[u8], target_width: u32, target_height: u32, focus_x: f32, focus_y: f32) -> Result<Vec<u8>, String> {
    let image = open_image(bytes)?;
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 {
        return Err("这张图片打不开".into());
    }
    let scale = (target_width as f32 / width as f32).max(target_height as f32 / height as f32);
    let scaled_width = ((width as f32) * scale).ceil().max(target_width as f32) as u32;
    let scaled_height = ((height as f32) * scale).ceil().max(target_height as f32) as u32;
    let resized = if scaled_width == width && scaled_height == height {
        image
    } else {
        image.resize_exact(scaled_width, scaled_height, image::imageops::FilterType::Lanczos3)
    };
    let extra_x = scaled_width.saturating_sub(target_width);
    let extra_y = scaled_height.saturating_sub(target_height);
    let focus_x = focus_x.clamp(0.0, 1.0);
    let focus_y = focus_y.clamp(0.0, 1.0);
    let x = ((extra_x as f32) * focus_x).round() as u32;
    let y = ((extra_y as f32) * focus_y).round() as u32;
    let cropped = image::imageops::crop_imm(
        &resized.to_rgb8(),
        x.min(extra_x),
        y.min(extra_y),
        target_width,
        target_height,
    )
    .to_image();
    encode_jpeg(&cropped, 92)
}

pub fn thumb_jpeg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = open_image(bytes)?;
    let (width, height) = (image.width(), image.height());
    let edge = width.max(height);
    let thumb = if edge > 480 {
        image.resize(
            ((width as f32) * 480.0 / edge as f32).max(1.0) as u32,
            ((height as f32) * 480.0 / edge as f32).max(1.0) as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    encode_jpeg(&thumb.to_rgb8(), 75)
}

pub fn to_jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    encode_jpeg(&image.to_rgb8(), quality)
}
