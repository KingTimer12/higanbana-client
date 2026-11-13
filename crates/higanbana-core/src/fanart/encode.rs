//! Redução e codificação das capas do fanart.tv.

use super::{FanartError, FanartResult};

/// Largura máxima da capa gravada em disco.
pub const COVER_WIDTH: u32 = 480;
/// Qualidade da codificação com perdas.
pub const COVER_QUALITY: f32 = 85.0;

/// Reduz para `COVER_WIDTH` (sem ampliar) e codifica. Extensão do arquivo resultante.
pub fn shrink(bytes: &[u8]) -> FanartResult<(Vec<u8>, &'static str)> {
    let img = image::load_from_memory(bytes).map_err(|e| FanartError::Image(e.to_string()))?;
    let img = if img.width() > COVER_WIDTH {
        let height = (u64::from(img.height()) * u64::from(COVER_WIDTH) / u64::from(img.width()))
            .max(1) as u32;
        img.resize_exact(COVER_WIDTH, height, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    encode(img)
}

#[cfg(feature = "webp-lossy")]
fn encode(img: image::DynamicImage) -> FanartResult<(Vec<u8>, &'static str)> {
    let rgba = image::DynamicImage::ImageRgba8(img.to_rgba8());
    let encoder = webp::Encoder::from_image(&rgba).map_err(|e| FanartError::Image(e.to_string()))?;
    Ok((encoder.encode(COVER_QUALITY).to_vec(), "webp"))
}

#[cfg(not(feature = "webp-lossy"))]
fn encode(img: image::DynamicImage) -> FanartResult<(Vec<u8>, &'static str)> {
    let rgb = img.to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, COVER_QUALITY as u8)
        .encode_image(&rgb)
        .map_err(|e| FanartError::Image(e.to_string()))?;
    Ok((out, "jpg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn dims(bytes: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(bytes).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn shrinks_to_480_wide() {
        let (out, _) = shrink(&png(1000, 1500)).unwrap();
        assert_eq!(dims(&out), (480, 720));
    }

    #[test]
    fn never_upscales() {
        let (out, _) = shrink(&png(300, 450)).unwrap();
        assert_eq!(dims(&out), (300, 450));
    }

    #[cfg(feature = "webp-lossy")]
    #[test]
    fn output_is_webp() {
        let (out, ext) = shrink(&png(600, 900)).unwrap();
        assert_eq!(ext, "webp");
        assert_eq!(&out[..4], b"RIFF");
        assert_eq!(&out[8..12], b"WEBP");
    }

    #[test]
    fn garbage_is_error() {
        assert!(matches!(shrink(b"nada"), Err(FanartError::Image(_))));
    }
}
