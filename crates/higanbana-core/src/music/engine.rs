//! Motor de reprodução: uma thread que recebe comandos, decodifica, reamostra
//! quando preciso, aplica o ganho e alimenta a saída, emendando faixas sem pausa.

use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tokio::runtime::Handle;

use crate::cloud::Cloud;

use super::{
    decode::{Decoder, Resampler, TrackInfo},
    gain::{self, LoudnessCache, Meter},
    output::{CpalSink, Sink, remap_channels},
    queue::{Queue, QueueTrack, Repeat},
    source::{BufferHandle, CloudFetcher, Fetcher, RangeSource},
};

/// Quanto áudio manter na fila da saída.
const BUFFER_SECS: f64 = 0.5;
/// Abrir a próxima faixa quando faltar isso para acabar a atual (ou quando ela já baixou toda).
const PREOPEN_SECS: f64 = 10.0;
/// Espera antes de pular uma faixa que falhou.
const ERROR_SKIP: Duration = Duration::from_secs(2);
/// Intervalo dos eventos de estado enquanto toca.
const STATE_EVERY: Duration = Duration::from_millis(250);
/// "Anterior" depois disso volta ao começo da faixa.
const PREV_RESTARTS_AFTER: f64 = 3.0;

pub type FetchFactory = Arc<dyn Fn(&QueueTrack) -> Arc<dyn Fetcher> + Send + Sync>;
/// Cria a saída para (taxa, canais) desejados; pode devolver outra taxa.
pub type SinkFactory = Box<dyn Fn(u32, u16) -> Result<Box<dyn Sink>, String> + Send>;
pub type StateFn = Box<dyn Fn(PlayerState) + Send>;

#[derive(Debug, Clone)]
pub enum Command {
    Play { album: String, tracks: Vec<QueueTrack>, index: usize, at: f64 },
    Pause,
    Resume,
    Toggle,
    Seek(f64),
    Next,
    Prev,
    Jump(usize),
    Volume(f32),
    Shuffle(bool),
    Repeat(Repeat),
    Normalize(bool),
    Stop,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FormatInfo {
    pub codec: String,
    pub bits: Option<u32>,
    pub sample_rate: u32,
    pub lossless: bool,
    pub resampled_to: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlayerState {
    pub album: Option<String>,
    pub index: Option<usize>,
    pub path: Option<String>,
    pub playing: bool,
    pub position: f64,
    pub duration: Option<f64>,
    pub buffered: f64,
    pub format: Option<FormatInfo>,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub normalize: bool,
    pub error: Option<String>,
    /// Abrindo a faixa (ainda sem som).
    pub loading: bool,
    /// O álbum acabou (repetir desligado).
    pub ended: bool,
    /// Título e artista das tags da faixa tocando (para as teclas de mídia).
    pub title: Option<String>,
    pub artist: Option<String>,
}

struct Opened {
    index: usize,
    decoder: Decoder,
    info: TrackInfo,
    handle: BufferHandle,
}
type OpenResult = Result<Opened, (usize, String)>;

struct Pending {
    rx: mpsc::Receiver<OpenResult>,
    index: usize,
    at: f64,
    gapless: bool,
}

enum Ctl {
    Seek(f64, u64),
}

enum Out {
    /// (geração do seek, amostras, canais)
    Block(u64, Vec<f32>, usize),
    End(u64),
}

/// Decodifica numa thread própria: esperar a rede nunca trava o motor.
struct Worker {
    ctl: mpsc::Sender<Ctl>,
    out: mpsc::Receiver<Out>,
    /// Geração atual; blocos de seeks anteriores são descartados.
    generation: u64,
}

impl Worker {
    fn spawn(mut decoder: Decoder, start_at: f64) -> Self {
        let (ctl, ctl_rx) = mpsc::channel::<Ctl>();
        let (tx, out) = mpsc::sync_channel::<Out>(4);
        std::thread::Builder::new()
            .name("music-decode".into())
            .spawn(move || {
                if start_at > 0.0 {
                    let _ = decoder.seek(start_at);
                }
                let mut generation = 0;
                loop {
                    loop {
                        match ctl_rx.try_recv() {
                            Ok(Ctl::Seek(s, g)) => {
                                let _ = decoder.seek(s);
                                generation = g;
                            }
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => return,
                        }
                    }
                    match decoder.next_block() {
                        Ok(Some(b)) => {
                            let ch = decoder.channels();
                            if tx.send(Out::Block(generation, b, ch)).is_err() {
                                return;
                            }
                        }
                        // Erro no meio da faixa conta como fim dela.
                        Ok(None) | Err(_) => {
                            if tx.send(Out::End(generation)).is_err() {
                                return;
                            }
                            // Fim: só um seek (voltar) reaproveita o decodificador.
                            match ctl_rx.recv() {
                                Ok(Ctl::Seek(s, g)) => {
                                    let _ = decoder.seek(s);
                                    generation = g;
                                }
                                Err(_) => return,
                            }
                        }
                    }
                }
            })
            .expect("thread de decodificação");
        Self { ctl, out, generation: 0 }
    }

    fn seek(&mut self, secs: f64) {
        self.generation += 1;
        let _ = self.ctl.send(Ctl::Seek(secs, self.generation));
    }
}

/// Faixa sendo decodificada.
struct Decoding {
    index: usize,
    worker: Worker,
    channels: usize,
    info: TrackInfo,
    handle: BufferHandle,
    resampler: Option<Resampler>,
    gain: f32,
    meter: Meter,
    key: String,
    offset: f64,
    decoded_frames: u64,
}

impl Drop for Decoding {
    /// Faixa abandonada (outra faixa, stop, seek para outra): para o download e a thread.
    fn drop(&mut self) {
        self.handle.cancel();
    }
}

/// Trecho já mandado para a saída: diz o que está tocando em cada ponto.
struct Segment {
    start: u64,
    index: usize,
    offset: f64,
    duration: Option<f64>,
    format: FormatInfo,
    handle: BufferHandle,
    path: String,
    title: Option<String>,
    artist: Option<String>,
}

#[derive(Clone)]
pub struct Engine {
    tx: mpsc::Sender<Command>,
}

impl Engine {
    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    /// Motor real: nuvem, saída padrão e cache de loudness em `cache_path`.
    pub fn spawn(rt: Handle, cloud: Arc<Cloud>, cache_path: Option<PathBuf>, on_state: StateFn) -> Self {
        let fetch: FetchFactory = Arc::new(move |t: &QueueTrack| {
            Arc::new(CloudFetcher { rt: rt.clone(), cloud: cloud.clone(), path: t.path.clone() }) as Arc<dyn Fetcher>
        });
        let sink: SinkFactory = Box::new(|rate, ch| CpalSink::open(rate, ch).map(|s| Box::new(s) as Box<dyn Sink>));
        Self::spawn_with(fetch, sink, on_state, cache_path)
    }

    pub fn spawn_with(fetch: FetchFactory, sink: SinkFactory, on_state: StateFn, cache_path: Option<PathBuf>) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("music-engine".into())
            .spawn(move || Core::new(fetch, sink, on_state, cache_path).run(rx))
            .expect("thread do player");
        Self { tx }
    }
}

struct Core {
    fetch: FetchFactory,
    make_sink: SinkFactory,
    on_state: StateFn,
    cache: LoudnessCache,
    cache_path: Option<PathBuf>,
    sink: Option<Box<dyn Sink>>,
    album: Option<String>,
    queue: Option<Queue>,
    decoding: Option<Decoding>,
    segments: VecDeque<Segment>,
    pushed: u64,
    pending: Option<Pending>,
    preopen: Option<(usize, mpsc::Receiver<OpenResult>)>,
    error: Option<(usize, String, Instant)>,
    /// Falhas seguidas; uma volta inteira na fila sem nada abrir para tudo.
    failures: usize,
    paused: bool,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    normalize: bool,
    ended: bool,
    last_state: Option<PlayerState>,
    last_emit: Instant,
}

/// Igual a `==`, ignorando posição e quanto já baixou (sem clonar os estados).
fn same_but_position(a: &PlayerState, b: &PlayerState) -> bool {
    let PlayerState {
        album,
        index,
        path,
        playing,
        position: _,
        duration,
        buffered: _,
        format,
        volume,
        shuffle,
        repeat,
        normalize,
        error,
        loading,
        ended,
        title,
        artist,
    } = a;
    *album == b.album
        && *index == b.index
        && *path == b.path
        && *playing == b.playing
        && *duration == b.duration
        && *format == b.format
        && *volume == b.volume
        && *shuffle == b.shuffle
        && *repeat == b.repeat
        && *normalize == b.normalize
        && *error == b.error
        && *loading == b.loading
        && *ended == b.ended
        && *title == b.title
        && *artist == b.artist
}

fn seed() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
}

impl Core {
    fn new(fetch: FetchFactory, make_sink: SinkFactory, on_state: StateFn, cache_path: Option<PathBuf>) -> Self {
        let cache = cache_path.as_deref().map(LoudnessCache::load).unwrap_or_default();
        Self {
            fetch,
            make_sink,
            on_state,
            cache,
            cache_path,
            sink: None,
            album: None,
            queue: None,
            decoding: None,
            segments: VecDeque::new(),
            pushed: 0,
            pending: None,
            preopen: None,
            error: None,
            failures: 0,
            paused: false,
            volume: 0.8,
            shuffle: false,
            repeat: Repeat::All,
            normalize: true,
            ended: false,
            last_state: None,
            last_emit: Instant::now(),
        }
    }

    fn run(mut self, rx: mpsc::Receiver<Command>) {
        loop {
            let busy = self.feed();
            let wait = if busy { Duration::ZERO } else { Duration::from_millis(10) };
            match rx.recv_timeout(wait) {
                Ok(c) => {
                    self.command(c);
                    while let Ok(c) = rx.try_recv() {
                        self.command(c);
                    }
                    self.emit(true);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            self.recover_output();
            self.poll_open();
            self.maybe_preopen();
            self.maybe_skip_error();
            self.emit(false);
        }
    }

    fn open_async(&self, index: usize, track: &QueueTrack) -> mpsc::Receiver<OpenResult> {
        let fetcher = (self.fetch)(track);
        let (size, ext) = (track.size, track.ext.clone());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let src = RangeSource::new(fetcher, size);
            let handle = src.handle();
            let r = Decoder::open(Box::new(src), &ext).map(|(decoder, info)| Opened { index, decoder, info, handle }).map_err(|e| (index, e));
            let _ = tx.send(r);
        });
        rx
    }

    /// Para tudo o que está tocando e abre a faixa atual da fila a partir de `at`.
    fn start_current(&mut self, at: f64) {
        // A faixa pedida já estava sendo aberta para emendar: reaproveita (sem outro pedido).
        let preopen = self.preopen.take();
        self.reset_playback();
        let Some((index, track)) = self.queue.as_ref().and_then(|q| q.current()).map(|(i, t)| (i, t.clone())) else { return };
        let rx = match preopen {
            Some((i, rx)) if i == index => rx,
            _ => self.open_async(index, &track),
        };
        self.pending = Some(Pending { rx, index, at, gapless: false });
    }

    fn reset_playback(&mut self) {
        if let Some(s) = &self.sink {
            s.clear();
        }
        self.save_measure();
        self.decoding = None;
        self.segments.clear();
        self.pending = None;
        self.preopen = None;
        self.error = None;
        self.ended = false;
    }

    fn album_keys(&self) -> Vec<String> {
        self.queue.as_ref().map_or_else(Vec::new, |q| q.tracks().iter().map(|t| gain::cache_key(&t.path, t.size)).collect())
    }

    fn install(&mut self, o: Opened, at: f64, gapless: bool) {
        let want_ch = o.info.channels.clamp(1, 2).max(2);
        let need_new = !gapless && self.sink.as_ref().is_none_or(|s| s.rate() != o.info.sample_rate);
        if self.sink.is_none() || need_new {
            match (self.make_sink)(o.info.sample_rate, want_ch) {
                Ok(s) if self.sink.is_none() || s.rate() == o.info.sample_rate => {
                    s.set_volume(self.volume * self.volume);
                    if self.paused {
                        s.pause();
                    }
                    self.sink = Some(s);
                }
                Ok(_) => {}
                Err(e) if self.sink.is_none() => {
                    self.error = Some((o.index, e, Instant::now() + Duration::from_secs(3600)));
                    return;
                }
                Err(_) => {}
            }
        }
        let sink = self.sink.as_ref().expect("saída aberta");
        let rate = sink.rate();
        let channels = o.decoder.channels();
        let resampler = (o.info.sample_rate != rate).then(|| Resampler::new(o.info.sample_rate, rate, channels));
        let keys = self.album_keys();
        let measured = self.cache.album_lufs(&keys).zip(self.cache.album_peak(&keys));
        let gain = gain::track_gain(&o.info.tags, measured, self.normalize);
        let track = self.queue.as_ref().and_then(|q| q.tracks().get(o.index)).cloned();
        let path = track.as_ref().map(|t| t.path.clone()).unwrap_or_default();
        let key = track.as_ref().map(|t| gain::cache_key(&t.path, t.size)).unwrap_or_default();
        self.segments.push_back(Segment {
            start: self.pushed,
            index: o.index,
            offset: at,
            duration: o.info.duration,
            format: FormatInfo {
                codec: o.info.codec.clone(),
                bits: o.info.bits,
                sample_rate: o.info.sample_rate,
                lossless: o.info.lossless,
                resampled_to: resampler.as_ref().map(|_| rate),
            },
            handle: o.handle.clone(),
            path,
            title: o.info.tags.title.clone(),
            artist: o.info.tags.artist.clone(),
        });
        self.error = None;
        self.failures = 0;
        self.decoding = Some(Decoding {
            index: o.index,
            meter: Meter::new(channels as u32, o.info.sample_rate),
            worker: Worker::spawn(o.decoder, at),
            channels,
            handle: o.handle,
            resampler,
            gain,
            key,
            offset: at,
            decoded_frames: 0,
            info: o.info,
        });
    }

    fn push(&mut self, samples: &[f32], from_ch: usize) {
        let Some(sink) = &self.sink else { return };
        let to = sink.channels() as usize;
        // Mesma disposição de canais: manda direto, sem cópia.
        let remapped;
        let s = if from_ch == to || from_ch == 0 {
            samples
        } else {
            remapped = remap_channels(samples, from_ch, to);
            &remapped[..]
        };
        self.pushed += (s.len() / to.max(1)) as u64;
        sink.push(s);
    }

    /// Decodifica um bloco se a saída precisa. Devolve se fez algo.
    fn feed(&mut self) -> bool {
        let Some(sink) = &self.sink else { return false };
        if sink.queued_frames() as f64 / sink.rate() as f64 > BUFFER_SECS {
            return false;
        }
        let Some(d) = &mut self.decoding else { return false };
        let ended = loop {
            match d.worker.out.try_recv() {
                Ok(Out::Block(g, block, ch)) if g == d.worker.generation => {
                    d.channels = ch;
                    d.meter.push(&block);
                    d.decoded_frames += (block.len() / ch.max(1)) as u64;
                    let mut s = match &mut d.resampler {
                        Some(r) => r.process(&block),
                        None => block,
                    };
                    let g = d.gain;
                    if g != 1.0 {
                        s.iter_mut().for_each(|x| *x *= g);
                    }
                    self.push(&s, ch);
                    return true;
                }
                Ok(Out::End(g)) if g == d.worker.generation => break true,
                // De antes de um seek: descarta.
                Ok(_) => continue,
                Err(mpsc::TryRecvError::Empty) => return false,
                Err(mpsc::TryRecvError::Disconnected) => break true,
            }
        };
        if ended {
            if let Some(r) = &mut d.resampler {
                let ch = d.channels;
                let g = d.gain;
                let mut s = r.flush();
                s.iter_mut().for_each(|x| *x *= g);
                self.push(&s, ch);
            }
            self.save_measure();
            self.decoding = None;
            self.track_ended();
        }
        true
    }

    fn save_measure(&mut self) {
        let Some(d) = &self.decoding else { return };
        if let Some(m) = d.meter.finish() {
            self.cache.tracks.insert(d.key.clone(), m);
            if let Some(p) = &self.cache_path {
                self.cache.save(p);
            }
        }
    }

    /// A faixa decodificando acabou: emenda a próxima.
    fn track_ended(&mut self) {
        let Some(next) = self.queue.as_mut().and_then(|q| q.advance()).map(|(i, t)| (i, t.clone())) else { return };
        let rx = match self.preopen.take() {
            Some((i, rx)) if i == next.0 => rx,
            _ => self.open_async(next.0, &next.1),
        };
        self.pending = Some(Pending { rx, index: next.0, at: 0.0, gapless: true });
        self.poll_open();
    }

    fn poll_open(&mut self) {
        let Some(p) = &self.pending else { return };
        let r = match p.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err((p.index, "falha ao abrir a faixa".into())),
        };
        let Pending { at, gapless, .. } = self.pending.take().unwrap();
        match r {
            Ok(o) => self.install(o, at, gapless),
            Err((index, e)) => {
                self.failures += 1;
                self.error = Some((index, e, Instant::now() + ERROR_SKIP));
            }
        }
    }

    fn maybe_preopen(&mut self) {
        if self.preopen.is_some() || self.pending.is_some() {
            return;
        }
        let Some(d) = &self.decoding else { return };
        let done = d.offset + d.decoded_frames as f64 / d.info.sample_rate as f64;
        let near_end = d.info.duration.is_some_and(|t| t - done < PREOPEN_SECS);
        if !(near_end || d.handle.buffered() >= 0.999) {
            return;
        }
        let Some((i, t)) = self.queue.as_ref().and_then(|q| q.peek_next()).map(|(i, t)| (i, t.clone())) else { return };
        if i == d.index {
            return; // repetir faixa: reabre no fim
        }
        let rx = self.open_async(i, &t);
        self.preopen = Some((i, rx));
    }

    fn maybe_skip_error(&mut self) {
        let Some((_, _, until)) = &self.error else { return };
        if Instant::now() < *until || self.pending.is_some() || self.decoding.is_some() {
            return;
        }
        let all_failed = self.queue.as_ref().is_none_or(|q| self.failures >= q.tracks().len());
        match self.queue.as_mut().filter(|_| !all_failed).and_then(|q| q.next()) {
            Some(_) => {
                let failures = self.failures;
                self.start_current(0.0);
                self.failures = failures;
            }
            None => {
                self.error = None;
                self.failures = 0;
                self.ended = true;
            }
        }
    }

    /// Saída perdida (dispositivo trocado/desconectado): reabre a faixa onde estava.
    fn recover_output(&mut self) {
        if self.sink.as_ref().is_none_or(|s| s.healthy()) {
            return;
        }
        let pos = self.position();
        let index = self.segments.front().map(|s| s.index);
        self.sink = None;
        if let (Some(i), Some(q)) = (index, &mut self.queue) {
            q.jump(i);
            self.start_current(pos);
        }
    }

    fn played(&self) -> u64 {
        self.pushed - self.sink.as_ref().map_or(0, |s| s.queued_frames()).min(self.pushed)
    }

    fn position(&self) -> f64 {
        let (Some(seg), Some(sink)) = (self.segments.front(), &self.sink) else { return 0.0 };
        seg.offset + self.played().saturating_sub(seg.start) as f64 / sink.rate() as f64
    }

    fn command(&mut self, c: Command) {
        if matches!(c, Command::Play { .. } | Command::Next | Command::Prev | Command::Jump(_)) {
            self.failures = 0;
        }
        match c {
            Command::Play { album, tracks, index, at } => {
                let mut q = Queue::new(tracks, index);
                q.repeat = self.repeat;
                if self.shuffle {
                    q.set_shuffle(true, seed());
                }
                self.queue = Some(q);
                self.album = Some(album);
                self.set_paused(false);
                self.start_current(at);
            }
            Command::Pause => self.set_paused(true),
            Command::Resume => self.resume(),
            Command::Toggle => {
                if self.paused || self.ended {
                    self.resume()
                } else {
                    self.set_paused(true)
                }
            }
            Command::Seek(secs) => self.seek(secs),
            Command::Next => {
                if self.queue.as_mut().and_then(|q| q.next()).is_some() {
                    self.start_current(0.0);
                }
            }
            Command::Prev => {
                if self.position() > PREV_RESTARTS_AFTER {
                    self.seek(0.0);
                } else if self.queue.as_mut().and_then(|q| q.prev()).is_some() {
                    self.start_current(0.0);
                }
            }
            Command::Jump(i) => {
                if let Some(q) = &mut self.queue {
                    q.jump(i);
                    self.start_current(0.0);
                }
            }
            Command::Volume(v) => {
                self.volume = v.clamp(0.0, 1.0);
                if let Some(s) = &self.sink {
                    s.set_volume(self.volume * self.volume);
                }
            }
            Command::Shuffle(on) => {
                self.shuffle = on;
                if let Some(q) = &mut self.queue {
                    q.set_shuffle(on, seed());
                }
                self.preopen = None;
            }
            Command::Repeat(r) => {
                self.repeat = r;
                if let Some(q) = &mut self.queue {
                    q.repeat = r;
                }
                self.preopen = None;
            }
            Command::Normalize(on) => self.normalize = on,
            Command::Stop => {
                self.reset_playback();
                self.queue = None;
                self.album = None;
            }
        }
    }

    fn set_paused(&mut self, p: bool) {
        self.paused = p;
        if let Some(s) = &self.sink {
            if p { s.pause() } else { s.play() }
        }
    }

    fn resume(&mut self) {
        if self.ended {
            // Álbum acabou: recomeça do início da fila.
            if let Some(q) = &mut self.queue {
                let first = q.order().first().copied().unwrap_or(0);
                q.jump(first);
                self.set_paused(false);
                self.start_current(0.0);
            }
            return;
        }
        self.set_paused(false);
    }

    fn seek(&mut self, secs: f64) {
        let Some(seg) = self.segments.front() else { return };
        let index = seg.index;
        let same = self.segments.len() == 1 && self.decoding.as_ref().is_some_and(|d| d.index == index);
        if !same {
            if let Some(q) = &mut self.queue {
                q.jump(index);
            }
            self.start_current(secs);
            return;
        }
        let d = self.decoding.as_mut().unwrap();
        d.worker.seek(secs);
        if d.resampler.is_some() {
            d.resampler = Some(Resampler::new(d.info.sample_rate, self.sink.as_ref().map_or(d.info.sample_rate, |s| s.rate()), d.channels));
        }
        d.offset = secs;
        d.decoded_frames = 0;
        if let Some(s) = &self.sink {
            s.clear();
        }
        self.preopen = None;
        let seg = self.segments.front_mut().unwrap();
        seg.start = self.pushed;
        seg.offset = secs;
    }

    fn state(&mut self) -> PlayerState {
        let played = self.played();
        while self.segments.len() > 1 && self.segments[1].start <= played {
            self.segments.pop_front();
        }
        // Fila acabou e a saída esvaziou.
        if self.album.is_some()
            && !self.ended
            && self.decoding.is_none()
            && self.pending.is_none()
            && self.error.is_none()
            && !self.segments.is_empty()
            && self.sink.as_ref().is_some_and(|s| s.queued_frames() == 0)
        {
            self.ended = true;
        }
        let seg = self.segments.front();
        let index = self.pending.as_ref().filter(|p| !p.gapless).map(|p| p.index).or(seg.map(|s| s.index)).or(self.error.as_ref().map(|e| e.0));
        PlayerState {
            album: self.album.clone(),
            index: self.album.as_ref().and(index),
            path: seg.map(|s| s.path.clone()),
            playing: self.album.is_some() && !self.paused && !self.ended && (seg.is_some() || self.pending.is_some()),
            position: if self.pending.as_ref().is_some_and(|p| !p.gapless) { self.pending.as_ref().unwrap().at } else { self.position() },
            duration: seg.and_then(|s| s.duration),
            buffered: seg.map_or(0.0, |s| s.handle.buffered()),
            format: seg.map(|s| s.format.clone()),
            volume: self.volume,
            shuffle: self.shuffle,
            repeat: self.repeat,
            normalize: self.normalize,
            error: self.error.as_ref().map(|e| e.1.clone()),
            loading: self.pending.as_ref().is_some_and(|p| !p.gapless),
            ended: self.ended,
            title: seg.and_then(|s| s.title.clone()),
            artist: seg.and_then(|s| s.artist.clone()),
        }
    }

    /// Emite o estado: na hora se algo além da posição mudou, senão a cada 250 ms.
    fn emit(&mut self, force: bool) {
        let s = self.state();
        let changed = self.last_state.as_ref().is_none_or(|l| !same_but_position(l, &s));
        let moving = self.last_state.as_ref() != Some(&s);
        if force || changed || (moving && self.last_emit.elapsed() >= STATE_EVERY) {
            self.last_emit = Instant::now();
            self.last_state = Some(s.clone());
            (self.on_state)(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::{decode::tests::wav, source::Fetcher};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    use std::time::{Duration, Instant};

    struct MemFetcher(Vec<u8>);
    impl Fetcher for MemFetcher {
        fn stream(&self, offset: u64, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String> {
            for c in self.0[offset as usize..].chunks(64 * 1024) {
                if !sink(c) {
                    break;
                }
            }
            Ok(())
        }
    }

    /// Saída falsa: consome na hora e conta as amostras.
    struct NullSink {
        pushed: Arc<AtomicUsize>,
    }
    impl Sink for NullSink {
        fn rate(&self) -> u32 {
            48000
        }
        fn channels(&self) -> u16 {
            2
        }
        fn push(&self, s: &[f32]) {
            self.pushed.fetch_add(s.len(), Ordering::SeqCst);
        }
        fn queued_frames(&self) -> u64 {
            0
        }
        fn clear(&self) {}
        fn pause(&self) {}
        fn play(&self) {}
        fn set_volume(&self, _: f32) {}
    }

    struct Rig {
        engine: Engine,
        pushed: Arc<AtomicUsize>,
        states: mpsc::Receiver<PlayerState>,
        tracks: Vec<QueueTrack>,
    }

    fn rig(files: Vec<Vec<u8>>) -> Rig {
        let tracks: Vec<QueueTrack> =
            files.iter().enumerate().map(|(i, f)| QueueTrack { path: format!("{i}.wav"), size: f.len() as u64, ext: ".wav".into() }).collect();
        let data: Arc<Vec<Vec<u8>>> = Arc::new(files);
        let fetch: FetchFactory = Arc::new(move |t: &QueueTrack| {
            let i: usize = t.path.trim_end_matches(".wav").parse().unwrap();
            Arc::new(MemFetcher(data[i].clone())) as Arc<dyn Fetcher>
        });
        let pushed = Arc::new(AtomicUsize::new(0));
        let p = pushed.clone();
        let sink: SinkFactory = Box::new(move |_, _| Ok(Box::new(NullSink { pushed: p.clone() }) as Box<dyn Sink>));
        let (tx, states) = mpsc::channel();
        let engine = Engine::spawn_with(fetch, sink, Box::new(move |s| drop(tx.send(s))), None);
        Rig { engine, pushed, states, tracks }
    }

    /// Espera um estado que satisfaça `f`.
    fn wait(r: &Rig, f: impl Fn(&PlayerState) -> bool) -> PlayerState {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let left = end.saturating_duration_since(Instant::now());
            let s = r.states.recv_timeout(left).expect("estado esperado não chegou");
            if f(&s) {
                return s;
            }
        }
    }

    fn play(r: &Rig, index: usize) {
        r.engine.send(Command::Repeat(Repeat::Off));
        r.engine.send(Command::Play { album: "A".into(), tracks: r.tracks.clone(), index, at: 0.0 });
    }

    #[test]
    fn plays_album_gapless_across_rates() {
        let r = rig(vec![wav(44100, 2, 1.0, 440.0), wav(48000, 2, 1.0, 440.0)]);
        play(&r, 0);
        wait(&r, |s| s.index == Some(1));
        let end = wait(&r, |s| !s.playing && s.album.is_some() && s.ended);
        assert_eq!(end.index, Some(1));
        let got = r.pushed.load(Ordering::SeqCst) as f64;
        let want = 2.0 * 48000.0 * 2.0;
        assert!((got - want).abs() / want < 0.02, "{got} vs {want}");
    }

    #[test]
    fn reports_format_and_resampling() {
        let r = rig(vec![wav(44100, 2, 1.0, 440.0)]);
        play(&r, 0);
        let s = wait(&r, |s| s.format.is_some());
        let f = s.format.unwrap();
        assert_eq!(f.codec, "WAV");
        assert_eq!(f.sample_rate, 44100);
        assert_eq!(f.resampled_to, Some(48000));
        assert!(s.duration.is_some_and(|d| (d - 1.0).abs() < 0.01));
    }

    #[test]
    fn next_and_prev_change_index() {
        let r = rig(vec![wav(48000, 2, 5.0, 440.0), wav(48000, 2, 5.0, 440.0), wav(48000, 2, 5.0, 440.0)]);
        play(&r, 0);
        wait(&r, |s| s.index == Some(0) && s.playing);
        r.engine.send(Command::Next);
        wait(&r, |s| s.index == Some(1));
        r.engine.send(Command::Jump(2));
        wait(&r, |s| s.index == Some(2));
        r.engine.send(Command::Prev);
        wait(&r, |s| s.index == Some(1));
    }

    #[test]
    fn stop_clears() {
        let r = rig(vec![wav(48000, 2, 5.0, 440.0)]);
        play(&r, 0);
        wait(&r, |s| s.playing);
        r.engine.send(Command::Stop);
        let s = wait(&r, |s| s.album.is_none());
        assert!(!s.playing);
        assert_eq!(s.index, None);
    }

    #[test]
    fn broken_track_reports_error_and_skips() {
        let r = rig(vec![b"isso nao e audio".to_vec(), wav(48000, 2, 0.5, 440.0)]);
        play(&r, 0);
        let s = wait(&r, |s| s.error.is_some());
        assert_eq!(s.index, Some(0));
        wait(&r, |s| s.index == Some(1) && s.playing);
    }

    /// Entrega o começo da faixa e depois trava (rede parada), até ser cancelado.
    struct StallFetcher(Vec<u8>, Arc<AtomicUsize>);
    impl Fetcher for StallFetcher {
        fn stream(&self, offset: u64, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String> {
            let end = (offset as usize + 64 * 1024).min(self.0.len());
            if offset as usize >= end || !sink(&self.0[offset as usize..end]) {
                return Ok(());
            }
            while sink(&[]) {
                std::thread::sleep(Duration::from_millis(20));
            }
            self.1.fetch_add(1, Ordering::SeqCst); // cancelado
            Ok(())
        }
    }

    #[test]
    fn commands_still_work_while_network_stalls() {
        let file = wav(48000, 2, 10.0, 440.0);
        let tracks = vec![QueueTrack { path: "0.wav".into(), size: file.len() as u64, ext: ".wav".into() }];
        let data = Arc::new(file);
        let cancelled = Arc::new(AtomicUsize::new(0));
        let c = cancelled.clone();
        let fetch: FetchFactory = Arc::new(move |_: &QueueTrack| Arc::new(StallFetcher((*data).clone(), c.clone())) as Arc<dyn Fetcher>);
        let pushed = Arc::new(AtomicUsize::new(0));
        let p = pushed.clone();
        let sink: SinkFactory = Box::new(move |_, _| Ok(Box::new(NullSink { pushed: p.clone() }) as Box<dyn Sink>));
        let (tx, states) = mpsc::channel();
        let engine = Engine::spawn_with(fetch, sink, Box::new(move |s| drop(tx.send(s))), None);
        let r = Rig { engine, pushed, states, tracks };
        play(&r, 0);
        wait(&r, |s| s.playing);
        std::thread::sleep(Duration::from_millis(300)); // decodificação já parada esperando a rede
        let t = Instant::now();
        r.engine.send(Command::Stop);
        wait(&r, |s| s.album.is_none());
        assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
        // O download parado é abandonado (não fica preso na rede).
        let end = Instant::now() + Duration::from_secs(2);
        while cancelled.load(Ordering::SeqCst) == 0 && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(cancelled.load(Ordering::SeqCst) >= 1, "download não foi cancelado");
    }

    /// Saída que "some" (fone desconectado) depois do primeiro bloco.
    struct DyingSink {
        pushes: Arc<AtomicUsize>,
    }
    impl Sink for DyingSink {
        fn rate(&self) -> u32 {
            48000
        }
        fn channels(&self) -> u16 {
            2
        }
        fn push(&self, s: &[f32]) {
            self.pushes.fetch_add(s.len(), Ordering::SeqCst);
        }
        fn queued_frames(&self) -> u64 {
            0
        }
        fn clear(&self) {}
        fn pause(&self) {}
        fn play(&self) {}
        fn set_volume(&self, _: f32) {}
        fn healthy(&self) -> bool {
            self.pushes.load(Ordering::SeqCst) == 0
        }
    }

    #[test]
    fn lost_output_device_is_reopened() {
        let file = wav(48000, 2, 3.0, 440.0);
        let tracks = vec![QueueTrack { path: "0.wav".into(), size: file.len() as u64, ext: ".wav".into() }];
        let data = Arc::new(file);
        let fetch: FetchFactory = Arc::new(move |_: &QueueTrack| Arc::new(MemFetcher((*data).clone())) as Arc<dyn Fetcher>);
        let opened = Arc::new(AtomicUsize::new(0));
        let o = opened.clone();
        let sink: SinkFactory = Box::new(move |_, _| {
            o.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(DyingSink { pushes: Arc::new(AtomicUsize::new(0)) }) as Box<dyn Sink>)
        });
        let (tx, states) = mpsc::channel();
        let engine = Engine::spawn_with(fetch, sink, Box::new(move |s| drop(tx.send(s))), None);
        let r = Rig { engine, pushed: Arc::new(AtomicUsize::new(0)), states, tracks };
        play(&r, 0);
        wait(&r, |s| s.playing);
        let end = Instant::now() + Duration::from_secs(5);
        while opened.load(Ordering::SeqCst) < 2 && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(opened.load(Ordering::SeqCst) >= 2, "a saída não foi reaberta");
    }

    /// Saída "cheia": o motor não decodifica mais nada (simula a faixa tocando devagar).
    struct FullSink;
    impl Sink for FullSink {
        fn rate(&self) -> u32 {
            48000
        }
        fn channels(&self) -> u16 {
            2
        }
        fn push(&self, _: &[f32]) {}
        fn queued_frames(&self) -> u64 {
            u64::MAX / 4
        }
        fn clear(&self) {}
        fn pause(&self) {}
        fn play(&self) {}
        fn set_volume(&self, _: f32) {}
    }

    struct CountingFetcher(Vec<u8>, String, Arc<Mutex<Vec<String>>>);
    impl Fetcher for CountingFetcher {
        fn stream(&self, offset: u64, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String> {
            self.2.lock().unwrap().push(format!("{}@{offset}", self.1));
            MemFetcher(self.0.clone()).stream(offset, sink)
        }
    }

    #[test]
    fn next_reuses_preopened_track() {
        let files = vec![wav(48000, 2, 30.0, 440.0), wav(48000, 2, 30.0, 440.0)];
        let tracks: Vec<QueueTrack> =
            files.iter().enumerate().map(|(i, f)| QueueTrack { path: format!("{i}.wav"), size: f.len() as u64, ext: ".wav".into() }).collect();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let c = calls.clone();
        let data = Arc::new(files);
        let fetch: FetchFactory = Arc::new(move |t: &QueueTrack| {
            let i: usize = t.path.trim_end_matches(".wav").parse().unwrap();
            Arc::new(CountingFetcher(data[i].clone(), t.path.clone(), c.clone())) as Arc<dyn Fetcher>
        });
        let sink: SinkFactory = Box::new(|_, _| Ok(Box::new(FullSink) as Box<dyn Sink>));
        let (tx, states) = mpsc::channel();
        let engine = Engine::spawn_with(fetch, sink, Box::new(move |s| drop(tx.send(s))), None);
        let r = Rig { engine, pushed: Arc::new(AtomicUsize::new(0)), states, tracks };
        play(&r, 0);
        wait(&r, |s| s.playing && s.index == Some(0));
        // Espera a pré-abertura da faixa 2 (a 1 já baixou toda).
        let end = Instant::now() + Duration::from_secs(3);
        while !calls.lock().unwrap().iter().any(|c| c.starts_with("1.wav")) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        r.engine.send(Command::Next);
        wait(&r, |s| s.index == Some(1) && s.playing);
        std::thread::sleep(Duration::from_millis(200));
        let n = calls.lock().unwrap().iter().filter(|c| c.starts_with("1.wav")).count();
        assert_eq!(n, 1, "{:?}", calls.lock().unwrap());
    }

    #[test]
    fn all_tracks_broken_stops_after_one_cycle() {
        let r = rig(vec![b"lixo 1".to_vec(), b"lixo 2".to_vec()]);
        r.engine.send(Command::Repeat(Repeat::All));
        r.engine.send(Command::Play { album: "A".into(), tracks: r.tracks.clone(), index: 0, at: 0.0 });
        let s = wait(&r, |s| s.ended);
        assert!(!s.playing);
        // E não volta a tentar depois de acabar.
        std::thread::sleep(Duration::from_millis(2500));
        while let Ok(s) = r.states.try_recv() {
            assert!(s.ended, "voltou a tentar: {s:?}");
        }
    }

    #[test]
    fn volume_and_settings_show_in_state() {
        let r = rig(vec![wav(48000, 2, 1.0, 440.0)]);
        r.engine.send(Command::Volume(0.3));
        r.engine.send(Command::Shuffle(true));
        r.engine.send(Command::Normalize(false));
        let s = wait(&r, |s| (s.volume - 0.3).abs() < 1e-6 && s.shuffle && !s.normalize);
        assert_eq!(s.repeat, Repeat::All);
    }
}
