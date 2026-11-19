//! AVIF em Rust puro: `avif-parse` tira o AV1 do contêiner e o `rav1d` (porta do
//! dav1d) decodifica. O `image` só lê AVIF com o dav1d em C, que não compila fácil
//! no Windows. Usado nas páginas de mangá que vêm em AVIF dentro do `.cbz`.

use std::{mem::MaybeUninit, ptr::NonNull};

use image::RgbaImage;
use rav1d::include::dav1d::{
    data::Dav1dData,
    dav1d::{Dav1dContext, Dav1dSettings},
    headers::{
        DAV1D_MC_BT2020_CL, DAV1D_MC_BT2020_NCL, DAV1D_MC_BT709, DAV1D_MC_IDENTITY, DAV1D_PIXEL_LAYOUT_I400,
        DAV1D_PIXEL_LAYOUT_I420, DAV1D_PIXEL_LAYOUT_I422,
    },
    picture::Dav1dPicture,
};
use rav1d::src::lib::{
    dav1d_close, dav1d_data_create, dav1d_data_unref, dav1d_default_settings, dav1d_get_picture, dav1d_open,
    dav1d_picture_unref, dav1d_send_data,
};

/// Começa com a caixa `ftyp` de marca `avif`/`avis`.
pub fn is_avif(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[4..8] == b"ftyp" && matches!(&bytes[8..12], b"avif" | b"avis")
}

/// Decodifica um AVIF estático para RGBA (com alfa, se houver).
#[hotpath::measure]
pub fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    let data = avif_parse::read_avif(&mut &bytes[..]).map_err(|e| format!("avif: {e}"))?;
    let color = Picture::decode(&data.primary_item)?;
    let mut rgba = color.to_rgba()?;
    if let Some(alpha) = &data.alpha_item {
        let a = Picture::decode(alpha)?;
        a.write_alpha(&mut rgba, data.premultiplied_alpha)?;
    }
    Ok(rgba)
}

/// Quadro decodificado: dono do `Dav1dPicture` e do contexto até ser descartado.
struct Picture {
    ctx: Option<Dav1dContext>,
    pic: Dav1dPicture,
}

impl Drop for Picture {
    fn drop(&mut self) {
        // SAFETY: `pic` veio de `dav1d_get_picture` e `ctx` de `dav1d_open`; nenhum foi solto ainda.
        unsafe {
            dav1d_picture_unref(Some(NonNull::from(&mut self.pic)));
            dav1d_close(Some(NonNull::from(&mut self.ctx)));
        }
    }
}

impl Picture {
    fn decode(obu: &[u8]) -> Result<Self, String> {
        if obu.is_empty() {
            return Err("avif: item vazio".into());
        }
        // SAFETY: chamadas na ordem da API do dav1d; todo ponteiro aponta para memória
        // local válida, e o que foi alocado pelo decodificador é solto aqui ou no `Drop`.
        unsafe {
            let mut settings = MaybeUninit::<Dav1dSettings>::uninit();
            dav1d_default_settings(NonNull::new_unchecked(settings.as_mut_ptr()));
            let mut settings = settings.assume_init();
            // Uma imagem só: sem fila de quadros, resposta na hora.
            settings.n_threads = 1;
            settings.max_frame_delay = 1;
            let mut ctx: Option<Dav1dContext> = None;
            let r = dav1d_open(Some(NonNull::from(&mut ctx)), Some(NonNull::from(&mut settings)));
            if r.0 != 0 || ctx.is_none() {
                return Err(format!("av1: não abriu o decodificador ({})", r.0));
            }
            let mut out = Picture { ctx, pic: Dav1dPicture::default() };

            let mut data = Dav1dData::default();
            let buf = dav1d_data_create(Some(NonNull::from(&mut data)), obu.len());
            if buf.is_null() {
                return Err("av1: sem memória".into());
            }
            std::ptr::copy_nonoverlapping(obu.as_ptr(), buf, obu.len());
            let sent = dav1d_send_data(out.ctx.clone(), Some(NonNull::from(&mut data)));
            dav1d_data_unref(Some(NonNull::from(&mut data)));
            if sent.0 != 0 {
                return Err(format!("av1: dados recusados ({})", sent.0));
            }
            let got = dav1d_get_picture(out.ctx.clone(), Some(NonNull::from(&mut out.pic)));
            if got.0 != 0 {
                return Err(format!("av1: sem quadro ({})", got.0));
            }
            Ok(out)
        }
    }

    fn size(&self) -> (usize, usize) {
        (self.pic.p.w.max(0) as usize, self.pic.p.h.max(0) as usize)
    }

    /// Amostra (x, y) do plano `i`, normalizada para 0..=255 (ou o `max` do bpc).
    fn plane(&self, i: usize) -> Result<Plane<'_>, String> {
        let p = &self.pic.p;
        let (w, h) = self.size();
        let (sx, sy) = match (i, p.layout) {
            (0, _) => (0, 0),
            (_, DAV1D_PIXEL_LAYOUT_I420) => (1, 1),
            (_, DAV1D_PIXEL_LAYOUT_I422) => (1, 0),
            _ => (0, 0),
        };
        let pw = (w + sx) >> sx;
        let ph = (h + sy) >> sy;
        let stride = self.pic.stride[if i == 0 { 0 } else { 1 }];
        let ptr = self.pic.data[i].ok_or("av1: plano faltando")?.as_ptr() as *const u8;
        if stride < 0 {
            return Err("av1: stride negativo".into());
        }
        let wide = p.bpc > 8;
        let row_bytes = pw * if wide { 2 } else { 1 };
        let stride = stride as usize;
        if stride < row_bytes {
            return Err("av1: stride menor que a linha".into());
        }
        // SAFETY: o dav1d garante `ph` linhas de `stride` bytes no plano.
        let bytes = unsafe { std::slice::from_raw_parts(ptr, stride * (ph - 1) + row_bytes) };
        Ok(Plane { bytes, stride, wide, sx, sy })
    }

    fn max(&self) -> f32 {
        ((1u32 << self.pic.p.bpc.clamp(8, 16)) - 1) as f32
    }

    fn to_rgba(&self) -> Result<RgbaImage, String> {
        let (w, h) = self.size();
        if w == 0 || h == 0 {
            return Err("av1: quadro vazio".into());
        }
        let max = self.max();
        // SAFETY: `seq_hdr` vem do quadro e vive enquanto ele existir.
        let seq = self.pic.seq_hdr.map(|s| unsafe { s.as_ref() });
        let full = seq.is_some_and(|s| s.color_range != 0);
        let mtrx = seq.map_or(0, |s| s.mtrx);
        let gray = self.pic.p.layout == DAV1D_PIXEL_LAYOUT_I400;
        let y = self.plane(0)?;
        let (u, v) = if gray { (None, None) } else { (Some(self.plane(1)?), Some(self.plane(2)?)) };
        // Coeficientes (Kr, Kb). Sem indicação, BT.601, como o libavif.
        let (kr, kb) = match mtrx {
            DAV1D_MC_BT709 => (0.2126, 0.0722),
            DAV1D_MC_BT2020_NCL | DAV1D_MC_BT2020_CL => (0.2627, 0.0593),
            _ => (0.299, 0.114),
        };
        let identity = mtrx == DAV1D_MC_IDENTITY;
        let kg = 1. - kr - kb;
        let (y_off, y_scale, c_scale) = if full {
            (0., 1. / max, 1. / max)
        } else {
            let s = max / 255.;
            (16. * s, 1. / (219. * s), 1. / (224. * s))
        };
        let mid = (max + 1.) / 2.;
        let mut out = RgbaImage::new(w as u32, h as u32);
        for (row, line) in out.rows_mut().enumerate() {
            for (col, px) in line.enumerate() {
                let yy = ((y.get(col, row) - y_off) * y_scale).clamp(0., 1.);
                let (r, g, b) = match (&u, &v) {
                    (Some(u), Some(v)) if identity => (v.get(col, row) / max, yy, u.get(col, row) / max),
                    (Some(u), Some(v)) => {
                        let cb = (u.get(col, row) - mid) * c_scale;
                        let cr = (v.get(col, row) - mid) * c_scale;
                        let r = yy + 2. * (1. - kr) * cr;
                        let b = yy + 2. * (1. - kb) * cb;
                        let g = (yy - kr * r - kb * b) / kg;
                        (r, g, b)
                    }
                    _ => (yy, yy, yy),
                };
                let q = |c: f32| (c.clamp(0., 1.) * 255. + 0.5) as u8;
                *px = image::Rgba([q(r), q(g), q(b), 255]);
            }
        }
        Ok(out)
    }

    fn write_alpha(&self, rgba: &mut RgbaImage, premultiplied: bool) -> Result<(), String> {
        let (w, h) = self.size();
        if (w as u32, h as u32) != rgba.dimensions() {
            return Err("avif: alfa de tamanho diferente".into());
        }
        let max = self.max();
        let a = self.plane(0)?;
        for (row, line) in rgba.rows_mut().enumerate() {
            for (col, px) in line.enumerate() {
                let alpha = (a.get(col, row) / max).clamp(0., 1.);
                px.0[3] = (alpha * 255. + 0.5) as u8;
                if premultiplied && alpha > 0. {
                    for c in &mut px.0[..3] {
                        *c = ((*c as f32 / alpha).min(255.)) as u8;
                    }
                }
            }
        }
        Ok(())
    }
}

struct Plane<'a> {
    bytes: &'a [u8],
    stride: usize,
    wide: bool,
    sx: usize,
    sy: usize,
}

impl Plane<'_> {
    fn get(&self, x: usize, y: usize) -> f32 {
        let (x, y) = (x >> self.sx, y >> self.sy);
        if self.wide {
            let i = y * self.stride + x * 2;
            u16::from_ne_bytes([self.bytes[i], self.bytes[i + 1]]) as f32
        } else {
            self.bytes[y * self.stride + x] as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder as _;

    #[test]
    fn detects_avif_signature() {
        let head = [0, 0, 0, 0x20, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f', 0, 0, 0, 0];
        assert!(is_avif(&head));
        assert!(!is_avif(b"\x89PNG\r\n\x1a\n...."));
    }

    fn encode(img: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut out, 10, 90)
            .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
            .unwrap();
        out
    }

    #[test]
    fn decodes_colors_back() {
        let mut img = RgbaImage::new(64, 48);
        for (x, _, px) in img.enumerate_pixels_mut() {
            *px = if x < 32 { image::Rgba([220, 30, 40, 255]) } else { image::Rgba([20, 90, 200, 255]) };
        }
        let bytes = encode(&img);
        assert!(is_avif(&bytes));
        let out = decode(&bytes).unwrap();
        assert_eq!(out.dimensions(), (64, 48));
        let close = |a: [u8; 4], b: [u8; 4]| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 12);
        assert!(close(out.get_pixel(8, 24).0, [220, 30, 40, 255]), "{:?}", out.get_pixel(8, 24));
        assert!(close(out.get_pixel(56, 24).0, [20, 90, 200, 255]), "{:?}", out.get_pixel(56, 24));
    }

    #[test]
    fn rejects_broken_avif() {
        let head = [0, 0, 0, 0x20, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f', 0, 0, 0, 0];
        assert!(decode(&head).is_err());
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use image::ImageEncoder as _;

    /// `cargo test --release -p higanbana-desktop avif_page_speed -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn avif_page_speed() {
        let img = RgbaImage::from_fn(1400, 2000, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x ^ y) % 256) as u8, 255]));
        let mut bytes = Vec::new();
        image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut bytes, 10, 70)
            .write_image(img.as_raw(), 1400, 2000, image::ExtendedColorType::Rgba8)
            .unwrap();
        let t = std::time::Instant::now();
        decode(&bytes).unwrap();
        println!("AVIF 1400x2000: {:?}", t.elapsed());
    }
}
