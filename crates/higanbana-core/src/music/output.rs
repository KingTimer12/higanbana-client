//! Saída de áudio (cpal / WASAPI). O volume é aplicado aqui, para responder na hora.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use cpal::{
    FromSample, SampleFormat, SampleRate, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

/// Para onde o motor manda as amostras (intercaladas, na taxa e nos canais da saída).
pub trait Sink {
    fn rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn push(&self, samples: &[f32]);
    /// Quadros ainda na fila, ainda não tocados.
    fn queued_frames(&self) -> u64;
    fn clear(&self);
    fn pause(&self);
    fn play(&self);
    /// Volume linear (0..1), já com a curva aplicada pelo motor.
    fn set_volume(&self, v: f32);
    /// `false` quando o dispositivo sumiu (fone desconectado etc.): o motor reabre a saída.
    fn healthy(&self) -> bool {
        true
    }
}

struct Shared {
    buf: Mutex<VecDeque<f32>>,
    paused: AtomicBool,
    volume: AtomicU32,
    failed: AtomicBool,
}

pub struct CpalSink {
    _stream: cpal::Stream,
    shared: Arc<Shared>,
    rate: u32,
    channels: u16,
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let shared_err = shared.clone();
    device.build_output_stream(
        config,
        move |out: &mut [T], _| {
            let vol = f32::from_bits(shared.volume.load(Ordering::Relaxed));
            if shared.paused.load(Ordering::Relaxed) {
                out.fill(T::from_sample(0.0));
                return;
            }
            // Trava só o tempo de copiar; o que faltar vira silêncio.
            let mut q = shared.buf.lock().unwrap();
            let n = out.len().min(q.len());
            for (o, s) in out.iter_mut().zip(q.drain(..n)) {
                *o = T::from_sample((s * vol).clamp(-1.0, 1.0));
            }
            drop(q);
            out[n..].fill(T::from_sample(0.0));
        },
        {
            let shared = shared_err;
            move |e| {
                tracing::error!(error = %e, "saída de áudio");
                shared.failed.store(true, Ordering::Relaxed);
            }
        },
        None,
    )
}

impl CpalSink {
    /// Abre a saída padrão, na taxa pedida se o dispositivo aceitar; senão na taxa dele.
    pub fn open(rate: u32, channels: u16) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("nenhuma saída de áudio")?;
        let want = SampleRate(rate);
        let exact = device.supported_output_configs().ok().and_then(|cfgs| {
            let mut v: Vec<_> = cfgs
                .filter(|c| c.channels() == channels && c.min_sample_rate() <= want && want <= c.max_sample_rate())
                .collect();
            v.sort_by_key(|c| c.sample_format() != SampleFormat::F32);
            v.into_iter().next().map(|c| c.with_sample_rate(want))
        });
        let supported = match exact {
            Some(c) => c,
            None => device.default_output_config().map_err(|e| e.to_string())?,
        };
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let shared = Arc::new(Shared { buf: Mutex::default(), paused: AtomicBool::new(false), volume: AtomicU32::new(1f32.to_bits()), failed: AtomicBool::new(false) });
        let s = shared.clone();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, s),
            SampleFormat::I16 => build::<i16>(&device, &config, s),
            SampleFormat::U16 => build::<u16>(&device, &config, s),
            SampleFormat::I32 => build::<i32>(&device, &config, s),
            SampleFormat::F64 => build::<f64>(&device, &config, s),
            other => return Err(format!("formato de saída não suportado: {other}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { _stream: stream, shared, rate: config.sample_rate.0, channels: config.channels })
    }
}

impl Sink for CpalSink {
    fn rate(&self) -> u32 {
        self.rate
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn push(&self, samples: &[f32]) {
        self.shared.buf.lock().unwrap().extend(samples);
    }
    fn queued_frames(&self) -> u64 {
        (self.shared.buf.lock().unwrap().len() / self.channels.max(1) as usize) as u64
    }
    fn clear(&self) {
        self.shared.buf.lock().unwrap().clear();
    }
    fn pause(&self) {
        self.shared.paused.store(true, Ordering::Relaxed);
    }
    fn play(&self) {
        self.shared.paused.store(false, Ordering::Relaxed);
    }
    fn set_volume(&self, v: f32) {
        self.shared.volume.store(v.to_bits(), Ordering::Relaxed);
    }
    fn healthy(&self) -> bool {
        !self.shared.failed.load(Ordering::Relaxed)
    }
}

/// Converte a disposição de canais (mono → estéreo, estéreo → 5.1 com o resto em silêncio etc.).
pub fn remap_channels(samples: &[f32], from: usize, to: usize) -> Vec<f32> {
    if from == to || from == 0 {
        return samples.to_vec();
    }
    let frames = samples.len() / from;
    let mut out = Vec::with_capacity(frames * to);
    for f in samples.chunks_exact(from) {
        for c in 0..to {
            out.push(match (from, c) {
                (1, _) if c < 2 => f[0],
                _ if c < from => f[c],
                _ => 0.0,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_mono_to_stereo_and_stereo_to_more() {
        assert_eq!(remap_channels(&[0.1, 0.2], 1, 2), vec![0.1, 0.1, 0.2, 0.2]);
        assert_eq!(remap_channels(&[0.1, 0.2], 2, 4), vec![0.1, 0.2, 0.0, 0.0]);
        assert_eq!(remap_channels(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6], 3, 2), vec![0.1, 0.2, 0.4, 0.5]);
    }
}
