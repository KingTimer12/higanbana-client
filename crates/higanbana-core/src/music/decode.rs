//! Decodificação (symphonia), tags e reamostragem (rubato).
//!
//! As amostras saem em `f32` intercaladas, na taxa e nos canais originais. A
//! reamostragem só entra quando a taxa da faixa difere da do dispositivo.

use std::io::{self, Read, Seek, SeekFrom};

use rubato::{
    Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{self, CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP3, CODEC_TYPE_NULL, CODEC_TYPE_VORBIS, DecoderOptions},
    errors::Error,
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
    io::{MediaSource, MediaSourceStream},
    meta::{MetadataOptions, MetadataRevision, StandardTagKey, StandardVisualKey},
    probe::Hint,
    units::Time,
};

#[derive(Debug, Clone, Default)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_gain: Option<f32>,
    pub album_gain: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_peak: Option<f32>,
    /// (mime, bytes) da capa embutida.
    pub cover: Option<(String, Vec<u8>)>,
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    /// "FLAC", "MP3", "WAV", "AAC", "Vorbis", "ALAC".
    pub codec: String,
    pub sample_rate: u32,
    pub bits: Option<u32>,
    pub channels: u16,
    pub duration: Option<f64>,
    pub lossless: bool,
    pub tags: Tags,
}

/// `"-6.48 dB"` → `-6.48`.
pub fn parse_gain(s: &str) -> Option<f32> {
    s.trim().trim_end_matches(|c: char| c.is_ascii_alphabetic() || c.is_whitespace()).trim().parse().ok()
}

fn apply_revision(tags: &mut Tags, rev: &MetadataRevision) {
    for t in rev.tags() {
        let v = t.value.to_string();
        let key = t.key.to_ascii_uppercase();
        match t.std_key {
            Some(StandardTagKey::TrackTitle) => tags.title = Some(v),
            Some(StandardTagKey::Artist) => tags.artist = Some(v),
            Some(StandardTagKey::Album) => tags.album = Some(v),
            Some(StandardTagKey::ReplayGainTrackGain) => tags.track_gain = parse_gain(&v),
            Some(StandardTagKey::ReplayGainAlbumGain) => tags.album_gain = parse_gain(&v),
            Some(StandardTagKey::ReplayGainTrackPeak) => tags.track_peak = parse_gain(&v),
            Some(StandardTagKey::ReplayGainAlbumPeak) => tags.album_peak = parse_gain(&v),
            // ID3 TXXX e afins chegam só com a chave.
            _ if key.ends_with("REPLAYGAIN_TRACK_GAIN") => tags.track_gain = parse_gain(&v),
            _ if key.ends_with("REPLAYGAIN_ALBUM_GAIN") => tags.album_gain = parse_gain(&v),
            _ if key.ends_with("REPLAYGAIN_TRACK_PEAK") => tags.track_peak = parse_gain(&v),
            _ if key.ends_with("REPLAYGAIN_ALBUM_PEAK") => tags.album_peak = parse_gain(&v),
            _ => {}
        }
    }
    let visuals = rev.visuals();
    let front = visuals.iter().find(|v| v.usage == Some(StandardVisualKey::FrontCover)).or(visuals.first());
    if let Some(v) = front {
        tags.cover = Some((v.media_type.clone(), v.data.to_vec()));
    }
}

fn codec_name(t: codecs::CodecType) -> (String, bool) {
    match t {
        CODEC_TYPE_FLAC => ("FLAC".into(), true),
        CODEC_TYPE_ALAC => ("ALAC".into(), true),
        CODEC_TYPE_MP3 => ("MP3".into(), false),
        CODEC_TYPE_AAC => ("AAC".into(), false),
        CODEC_TYPE_VORBIS => ("Vorbis".into(), false),
        _ => {
            let short = symphonia::default::get_codecs().get_codec(t).map(|d| d.short_name).unwrap_or("?");
            if short.starts_with("pcm") { ("WAV".into(), true) } else { (short.to_ascii_uppercase(), false) }
        }
    }
}

struct Opened {
    format: Box<dyn FormatReader>,
    track: u32,
    info: TrackInfo,
}

fn open_format(src: Box<dyn MediaSource>, ext_hint: &str) -> Result<Opened, String> {
    let mss = MediaSourceStream::new(src, Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext_hint.trim_start_matches('.'));
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
        .map_err(|e| format!("formato não suportado: {e}"))?;
    let mut tags = Tags::default();
    if let Some(rev) = probed.metadata.get().as_ref().and_then(|m| m.current().cloned()) {
        apply_revision(&mut tags, &rev);
    }
    let mut format = probed.format;
    if let Some(rev) = format.metadata().current() {
        apply_revision(&mut tags, rev);
    }
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("nenhuma faixa de áudio no arquivo")?;
    let p = &track.codec_params;
    let sample_rate = p.sample_rate.ok_or("taxa de amostragem desconhecida")?;
    let (codec, lossless) = codec_name(p.codec);
    let info = TrackInfo {
        codec,
        sample_rate,
        bits: p.bits_per_sample,
        channels: p.channels.map(|c| c.count() as u16).unwrap_or(2),
        duration: p.n_frames.map(|n| n as f64 / sample_rate as f64),
        lossless,
        tags,
    };
    Ok(Opened { track: track.id, format, info })
}

pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn codecs::Decoder>,
    track: u32,
    buf: Option<SampleBuffer<f32>>,
    channels: usize,
    /// Depois de um seek: descartar amostras antes deste timestamp.
    skip_until: Option<u64>,
}

impl Decoder {
    pub fn open(src: Box<dyn MediaSource>, ext_hint: &str) -> Result<(Self, TrackInfo), String> {
        let o = open_format(src, ext_hint)?;
        let params = o.format.tracks().iter().find(|t| t.id == o.track).map(|t| t.codec_params.clone()).ok_or("faixa sumiu")?;
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| format!("codec não suportado: {e}"))?;
        let channels = o.info.channels as usize;
        Ok((Self { format: o.format, decoder, track: o.track, buf: None, channels, skip_until: None }, o.info))
    }

    /// Próximo bloco intercalado em f32; `None` no fim.
    pub fn next_block(&mut self) -> Result<Option<Vec<f32>>, String> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(Error::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
                Err(Error::ResetRequired) => return Ok(None),
                Err(e) => return Err(e.to_string()),
            };
            if packet.track_id() != self.track {
                continue;
            }
            let audio = match self.decoder.decode(&packet) {
                Ok(a) => a,
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(e.to_string()),
            };
            let frames = audio.frames();
            if frames == 0 {
                continue;
            }
            let spec = *audio.spec();
            self.channels = spec.channels.count();
            if self.buf.as_ref().is_none_or(|b| b.capacity() < frames) {
                self.buf = Some(SampleBuffer::new(frames as u64, spec));
            }
            let buf = self.buf.as_mut().unwrap();
            buf.copy_interleaved_ref(audio);
            let mut samples = buf.samples();
            if let Some(until) = self.skip_until {
                let ts = packet.ts();
                if ts + frames as u64 <= until {
                    continue;
                }
                let drop = until.saturating_sub(ts) as usize * self.channels;
                samples = &samples[drop.min(samples.len())..];
                self.skip_until = None;
            }
            return Ok(Some(samples.to_vec()));
        }
    }

    pub fn seek(&mut self, secs: f64) -> Result<(), String> {
        let to = self
            .format
            .seek(SeekMode::Accurate, SeekTo::Time { time: Time::from(secs.max(0.0)), track_id: Some(self.track) })
            .map_err(|e| e.to_string())?;
        self.decoder.reset();
        self.skip_until = Some(to.required_ts);
        Ok(())
    }

    pub fn channels(&self) -> usize {
        self.channels
    }
}

/// Só o começo do arquivo; ler além devolve fim de arquivo.
struct PrefixSource {
    data: Vec<u8>,
    total: u64,
    pos: u64,
}

impl Read for PrefixSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let start = (self.pos as usize).min(self.data.len());
        let n = (self.data.len() - start).min(out.len());
        out[..n].copy_from_slice(&self.data[start..start + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for PrefixSource {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.pos = match to {
            SeekFrom::Start(p) => p,
            SeekFrom::End(d) => (self.total as i64 + d).max(0) as u64,
            SeekFrom::Current(d) => (self.pos as i64 + d).max(0) as u64,
        };
        Ok(self.pos)
    }
}

impl MediaSource for PrefixSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.total)
    }
}

/// Tamanho do cabeçalho ID3v2 no começo (0 se não houver).
pub fn id3_len(b: &[u8]) -> usize {
    if b.len() < 10 || &b[..3] != b"ID3" {
        return 0;
    }
    let size = b[6..10].iter().fold(0usize, |acc, &x| (acc << 7) | (x & 0x7f) as usize);
    let footer = if b[5] & 0x10 != 0 { 10 } else { 0 };
    10 + size + footer
}

/// Quantos bytes do começo o arquivo precisa para ler tags e duração (capa
/// embutida grande no FLAC ou no ID3 passa dos 256 KiB). `None` = o que veio basta.
pub fn needed_prefix(head: &[u8]) -> Option<u64> {
    if head.starts_with(b"ID3") {
        return Some(id3_len(head) as u64 + 64 * 1024);
    }
    if !head.starts_with(b"fLaC") {
        return None;
    }
    let mut pos = 4usize;
    loop {
        let Some(h) = head.get(pos..pos + 4) else { return Some(pos as u64 + 4) };
        let len = u32::from_be_bytes([0, h[1], h[2], h[3]]) as usize;
        pos += 4 + len;
        if h[0] & 0x80 != 0 {
            return Some(pos as u64 + 16 * 1024);
        }
    }
}

/// Bitrate do primeiro quadro MPEG Layer III encontrado.
pub fn mp3_bitrate(b: &[u8]) -> Option<u32> {
    const V1: [u32; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
    const V2: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    (0..b.len().saturating_sub(3)).find_map(|i| {
        if b[i] != 0xFF || b[i + 1] & 0xE0 != 0xE0 {
            return None;
        }
        let version = (b[i + 1] >> 3) & 3;
        let layer = (b[i + 1] >> 1) & 3;
        let idx = (b[i + 2] >> 4) as usize;
        if layer != 1 || version == 1 || idx == 0 || idx == 15 {
            return None;
        }
        Some(if version == 3 { V1[idx] } else { V2[idx] } * 1000)
    })
}

pub fn estimate_mp3_secs(audio_bytes: u64, bitrate: u32) -> f64 {
    audio_bytes as f64 * 8.0 / bitrate as f64
}

/// Tags e duração a partir do começo do arquivo e do tamanho total (sem baixar o resto).
pub fn probe_prefix(prefix: Vec<u8>, total_len: u64, ext_hint: &str) -> Result<TrackInfo, String> {
    let head = prefix[..prefix.len().min(64 * 1024)].to_vec();
    let src = PrefixSource { data: prefix, total: total_len, pos: 0 };
    let mut info = open_format(Box::new(src), ext_hint)?.info;
    if info.duration.is_none() && info.codec == "MP3" {
        let start = id3_len(&head);
        if let Some(br) = head.get(start..).and_then(mp3_bitrate) {
            info.duration = Some(estimate_mp3_secs(total_len.saturating_sub(start as u64), br));
        }
    }
    Ok(info)
}

/// Reamostrador de alta qualidade (sinc 256, Blackman-Harris²).
pub struct Resampler {
    inner: SincFixedIn<f32>,
    channels: usize,
    pending: Vec<Vec<f32>>,
    /// Saída reaproveitada entre blocos (sem alocar a cada chamada).
    out: Vec<Vec<f32>>,
    ratio: f64,
    frames_in: u64,
    frames_out: u64,
}

impl Resampler {
    pub fn new(from: u32, to: u32, channels: usize) -> Self {
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let ratio = to as f64 / from as f64;
        let inner = SincFixedIn::new(ratio, 1.0, params, 1024, channels).expect("parâmetros do reamostrador");
        let out = inner.output_buffer_allocate(true);
        // A rubato 0.16 já compensa o atraso do filtro: a saída começa alinhada com a entrada.
        Self { inner, channels, pending: vec![Vec::new(); channels], out, ratio, frames_in: 0, frames_out: 0 }
    }

    /// Intercala os `frames` primeiros quadros de `self.out` em `res`.
    fn emit(&mut self, frames: usize, limit: Option<u64>, res: &mut Vec<f32>) {
        let mut take = frames;
        if let Some(max) = limit {
            take = take.min(max.saturating_sub(self.frames_out) as usize);
        }
        self.frames_out += take as u64;
        res.reserve(take * self.channels);
        for i in 0..take {
            for ch in &self.out {
                res.push(ch[i]);
            }
        }
    }

    pub fn process(&mut self, interleaved: &[f32]) -> Vec<f32> {
        let frames = interleaved.len() / self.channels;
        for p in &mut self.pending {
            p.reserve(frames);
        }
        for (i, s) in interleaved.iter().enumerate() {
            self.pending[i % self.channels].push(*s);
        }
        self.frames_in += frames as u64;
        let mut res = Vec::new();
        while self.pending[0].len() >= self.inner.input_frames_next() {
            let need = self.inner.input_frames_next();
            // A rubato lê só os `need` primeiros quadros: passa a fila direto, sem copiar.
            let (_, n) = self.inner.process_into_buffer(&self.pending, &mut self.out, None).expect("reamostragem");
            for p in &mut self.pending {
                p.drain(..need);
            }
            self.emit(n, None, &mut res);
        }
        res
    }

    /// Fim da faixa: processa o resto e devolve só o que corresponde à entrada.
    pub fn flush(&mut self) -> Vec<f32> {
        let expected = (self.frames_in as f64 * self.ratio).round() as u64;
        let mut res = Vec::new();
        let rest: Vec<Vec<f32>> = std::mem::take(&mut self.pending);
        self.pending = vec![Vec::new(); self.channels];
        let (_, n) = self.inner.process_partial_into_buffer(Some(&rest), &mut self.out, None).expect("reamostragem");
        self.emit(n, Some(expected), &mut res);
        while self.frames_out < expected {
            let (_, n) = self.inner.process_partial_into_buffer::<Vec<f32>, _>(None, &mut self.out, None).expect("reamostragem");
            if n == 0 {
                break;
            }
            self.emit(n, Some(expected), &mut res);
        }
        res
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Cursor;
    use symphonia::core::io::ReadOnlySource;

    /// WAV PCM 16-bit em memória com um seno.
    pub fn wav(rate: u32, channels: u16, secs: f64, freq: f64) -> Vec<u8> {
        let frames = (rate as f64 * secs) as u32;
        let data_len = frames * channels as u32 * 2;
        let mut out = Vec::new();
        out.extend(b"RIFF");
        out.extend((36 + data_len).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(channels.to_le_bytes());
        out.extend(rate.to_le_bytes());
        out.extend((rate * channels as u32 * 2).to_le_bytes());
        out.extend((channels * 2).to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend(data_len.to_le_bytes());
        for i in 0..frames {
            let v = ((i as f64 / rate as f64 * freq * std::f64::consts::TAU).sin() * 0.5 * i16::MAX as f64) as i16;
            for _ in 0..channels {
                out.extend(v.to_le_bytes());
            }
        }
        out
    }

    fn src(bytes: Vec<u8>) -> Box<dyn MediaSource> {
        Box::new(Cursor::new(bytes))
    }

    fn drain(d: &mut Decoder) -> usize {
        let mut n = 0;
        while let Some(b) = d.next_block().unwrap() {
            n += b.len();
        }
        n
    }

    #[test]
    fn decodes_wav_samples_and_info() {
        let (mut d, info) = Decoder::open(src(wav(44100, 2, 1.0, 440.0)), "wav").unwrap();
        assert_eq!(info.codec, "WAV");
        assert_eq!(info.bits, Some(16));
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert!(info.lossless);
        assert!((info.duration.unwrap() - 1.0).abs() < 0.01);
        assert_eq!(drain(&mut d), 88200);
    }

    #[test]
    fn seek_lands_near_target() {
        let (mut d, _) = Decoder::open(src(wav(44100, 2, 1.0, 440.0)), "wav").unwrap();
        d.seek(0.5).unwrap();
        let left = drain(&mut d) as f64 / 2.0 / 44100.0;
        assert!((left - 0.5).abs() < 0.01, "{left}");
    }

    #[test]
    fn resample_keeps_duration() {
        let (mut d, _) = Decoder::open(src(wav(44100, 2, 1.0, 440.0)), "wav").unwrap();
        let mut r = Resampler::new(44100, 48000, 2);
        let mut out = 0;
        while let Some(b) = d.next_block().unwrap() {
            out += r.process(&b).len();
        }
        out += r.flush().len();
        assert!((out as f64 - 96000.0).abs() / 96000.0 < 0.01, "{out}");
    }

    #[test]
    fn resampled_sine_keeps_pitch() {
        // 1 kHz a 44,1 → 48 kHz continua 1 kHz: conta cruzamentos por zero.
        let (mut d, _) = Decoder::open(src(wav(44100, 1, 1.0, 1000.0)), "wav").unwrap();
        let mut r = Resampler::new(44100, 48000, 1);
        let mut s = Vec::new();
        while let Some(b) = d.next_block().unwrap() {
            s.extend(r.process(&b));
        }
        s.extend(r.flush());
        let crossings = s.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((crossings as i64 - 1000).abs() <= 3, "{crossings}");
    }

    #[test]
    fn probe_prefix_reads_wav_duration() {
        let full = wav(44100, 2, 2.0, 440.0);
        let total = full.len() as u64;
        let info = probe_prefix(full[..4096].to_vec(), total, "wav").unwrap();
        assert!((info.duration.unwrap() - 2.0).abs() < 0.01);
    }

    #[test]
    fn parses_replaygain_strings() {
        assert_eq!(parse_gain("-6.48 dB"), Some(-6.48));
        assert_eq!(parse_gain("+1.2 dB"), Some(1.2));
        assert_eq!(parse_gain("x"), None);
    }

    #[test]
    fn mp3_duration_from_bitrate() {
        // Cabeçalho MPEG-1 Layer III, 128 kbps, 44,1 kHz: FF FB 90 00.
        let mut b = vec![0u8; 64];
        b[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        assert_eq!(mp3_bitrate(&b), Some(128_000));
        // 128 kbps, 1.6 MB de áudio → 100 s.
        let _ = ReadOnlySource::new(Cursor::new(Vec::<u8>::new()));
        assert_eq!(estimate_mp3_secs(1_600_000, 128_000), 100.0);
    }

    #[test]
    fn needed_prefix_covers_big_flac_picture() {
        let mut b = b"fLaC".to_vec();
        b.extend([0x00, 0, 0, 34]);
        b.extend([0u8; 34]);
        b.extend([0x86, 0x07, 0xA1, 0x20]); // PICTURE, último bloco, 500000 bytes
        let end = 4 + 4 + 34 + 4 + 500_000;
        assert_eq!(needed_prefix(&b), Some(end + 16 * 1024));
        assert_eq!(needed_prefix(&b[..6]), Some(8));
        assert_eq!(needed_prefix(b"RIFF...."), None);
    }

    #[test]
    fn needed_prefix_covers_big_id3() {
        let mut b = vec![0u8; 20];
        b[..10].copy_from_slice(&[b'I', b'D', b'3', 4, 0, 0, 0, 0x20, 0, 0]); // 512 KiB
        assert_eq!(needed_prefix(&b), Some(10 + 512 * 1024 + 64 * 1024));
    }

    #[test]
    fn id3_size_is_skipped() {
        let mut b = vec![0u8; 20];
        b[..10].copy_from_slice(&[b'I', b'D', b'3', 4, 0, 0, 0, 0, 1, 0]);
        assert_eq!(id3_len(&b), 10 + 128);
    }
}

