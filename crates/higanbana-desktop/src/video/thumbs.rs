//! Miniaturas da barra de posição: uma segunda instância do mpv, sem janela
//! (`vo=null`), abre o mesmo arquivo, pula para o ponto sob o mouse e grava um
//! quadro pequeno em JPEG numa pasta temporária.
//!
//! Uma miniatura por vez: se o mouse andar enquanto uma sai, só a última posição
//! pedida é gerada depois. O tempo é arredondado em faixas de [`STEP`] segundos
//! e o `seek` vai ao quadro-chave, então cada faixa custa um pedaço pequeno do
//! arquivo e é feita uma vez só.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::channel::mpsc::UnboundedReceiver;

use super::mpv::{EndReason, Mpv, MpvEvent};

/// Largura da faixa de tempo de cada miniatura, em segundos.
pub const STEP: f64 = 10.;
/// Largura do quadro gravado, em pixels.
const WIDTH: u32 = 320;

/// Faixa do tempo `t` (segundos).
pub fn bucket(t: f64) -> u32 {
    (t.max(0.) / STEP).floor() as u32
}

/// Quantas faixas para cada lado da que está sob o mouse são geradas de
/// antemão, enquanto o gerador está livre.
const AHEAD: u32 = 3;

/// Faixa ainda sem miniatura mais perto de `center` (ele mesmo, depois a
/// seguinte, a anterior, a 2ª seguinte…), dentro de `0..=last` e do raio `radius`.
fn nearest_missing(center: u32, last: u32, radius: u32, have: impl Fn(u32) -> bool) -> Option<u32> {
    (0..=radius)
        .flat_map(|d| [center.checked_add(d), (d > 0).then(|| center.checked_sub(d)).flatten()])
        .flatten()
        .filter(|b| *b <= last)
        .find(|b| !have(*b))
}

fn options() -> Vec<(&'static str, String)> {
    vec![
        ("vo", "null".into()),
        ("pause", "yes".into()),
        ("keep-open", "yes".into()),
        ("audio", "no".into()),
        ("sid", "no".into()),
        ("hwdec", "no".into()),
        ("hr-seek", "no".into()),
        // Uma thread e sem filtros caros: só um quadro-chave por vez, sem disputar CPU com o vídeo.
        ("vd-lavc-threads", "1".into()),
        ("vd-lavc-skiploopfilter", "all".into()),
        ("vd-lavc-fast", "yes".into()),
        // Abrir sem sondar muito além do cabeçalho: cada byte vem da nuvem.
        ("demuxer-lavf-probesize", "524288".into()),
        ("demuxer-lavf-analyzeduration", "0.5".into()),
        ("vf", format!("scale=w={WIDTH}:h=-2")),
        // Só o que o quadro pede: sem ler à frente.
        ("cache", "no".into()),
        ("demuxer-readahead-secs", "0".into()),
        ("demuxer-max-bytes", "4MiB".into()),
        ("screenshot-format", "jpg".into()),
        ("screenshot-jpeg-quality", "80".into()),
        ("config", "no".into()),
        ("load-scripts", "no".into()),
        ("ytdl", "no".into()),
        ("input-default-bindings", "no".into()),
    ]
}

/// Quadro a gravar: rode [`Shot::run`] fora da thread da interface e devolva o
/// resultado a [`Thumbs::finish`].
pub struct Shot {
    pub bucket: u32,
    mpv: Arc<Mpv>,
    path: PathBuf,
}

impl Shot {
    pub fn run(&self) -> bool {
        self.mpv.command(&["screenshot-to-file", &self.path.to_string_lossy(), "video"]).is_ok() && self.path.exists()
    }
}

pub struct Thumbs {
    mpv: Arc<Mpv>,
    dir: PathBuf,
    /// Primeiro quadro já saiu: dá para pular sem confundir com a abertura.
    ready: bool,
    busy: Option<u32>,
    want: Option<u32>,
    /// Última faixa pedida: em volta dela se gera de antemão.
    near: Option<u32>,
    /// Última faixa que existe no arquivo (0 até saber a duração).
    last: u32,
    done: HashMap<u32, Arc<Path>>,
}

impl Thumbs {
    /// Abre `url` numa instância sem janela; as miniaturas vão para `dir`.
    pub fn open(url: &str, dir: PathBuf) -> Result<(Self, UnboundedReceiver<MpvEvent>), String> {
        Self::open_with(url, dir, &[])
    }

    /// Como [`Thumbs::open`], com opções extras (as de `extra` valem sobre as padrão).
    fn open_with(url: &str, dir: PathBuf, extra: &[(&'static str, String)]) -> Result<(Self, UnboundedReceiver<MpvEvent>), String> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut opts = options();
        opts.extend(extra.iter().cloned());
        let (mpv, rx) = Mpv::new(&opts, &[])?;
        mpv.command(&["loadfile", url])?;
        Ok((Self { mpv: Arc::new(mpv), dir, ready: false, busy: None, want: None, near: None, last: 0, done: HashMap::new() }, rx))
    }

    /// Duração do arquivo em segundos: limita o que se gera de antemão.
    pub fn set_len(&mut self, seconds: f64) {
        self.last = bucket(seconds);
    }

    /// Miniatura pronta da faixa.
    pub fn get(&self, bucket: u32) -> Option<Arc<Path>> {
        self.done.get(&bucket).cloned()
    }

    /// Pede a faixa (substitui o pedido anterior que ainda não começou).
    pub fn want(&mut self, bucket: u32) {
        self.near = Some(bucket);
        self.want = (!self.done.contains_key(&bucket)).then_some(bucket);
        self.pump();
    }

    /// Evento da instância: devolve o quadro a gravar quando o `seek` terminou.
    pub fn on_event(&mut self, e: MpvEvent) -> Option<Shot> {
        match e {
            MpvEvent::Restart if !self.ready => {
                self.ready = true;
                self.pump();
                None
            }
            MpvEvent::Restart => self.busy.map(|b| Shot { bucket: b, mpv: self.mpv.clone(), path: self.dir.join(format!("{b}.jpg")) }),
            MpvEvent::EndFile(EndReason::Error) => {
                // Arquivo não abriu: fica sem miniaturas.
                self.ready = false;
                self.busy = None;
                None
            }
            _ => None,
        }
    }

    /// Resultado de um [`Shot`]; começa o próximo pedido, se houver.
    pub fn finish(&mut self, bucket: u32, ok: bool) {
        if ok {
            self.done.insert(bucket, self.dir.join(format!("{bucket}.jpg")).into());
        }
        if self.busy == Some(bucket) {
            self.busy = None;
        }
        if self.want == Some(bucket) {
            self.want = None;
        }
        self.pump();
    }

    fn pump(&mut self) {
        if !self.ready || self.busy.is_some() {
            return;
        }
        // O que está sob o mouse primeiro; livre, as vizinhas.
        let next = self
            .want
            .filter(|b| !self.done.contains_key(b))
            .or_else(|| self.near.and_then(|c| nearest_missing(c, self.last, AHEAD, |b| self.done.contains_key(&b))));
        let Some(b) = next else { return };
        if self.mpv.command(&["seek", &(f64::from(b) * STEP).to_string(), "absolute+keyframes"]).is_ok() {
            self.busy = Some(b);
        }
    }
}

impl Drop for Thumbs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt as _;

    #[test]
    fn nearest_missing_goes_outward_forward_first() {
        let have = |set: &[u32]| {
            let set = set.to_vec();
            move |b: u32| set.contains(&b)
        };
        assert_eq!(nearest_missing(5, 100, 3, have(&[])), Some(5));
        assert_eq!(nearest_missing(5, 100, 3, have(&[5])), Some(6));
        assert_eq!(nearest_missing(5, 100, 3, have(&[5, 6])), Some(4));
        assert_eq!(nearest_missing(5, 100, 3, have(&[5, 6, 4])), Some(7));
        assert_eq!(nearest_missing(5, 100, 3, have(&[2, 3, 4, 5, 6, 7, 8])), None, "fora do raio não gera");
    }

    #[test]
    fn nearest_missing_stays_inside_the_file() {
        let none = |_: u32| false;
        assert_eq!(nearest_missing(0, 2, 3, |b| b == 0), Some(1));
        assert_eq!(nearest_missing(2, 2, 3, |b| b == 2), Some(1), "sem faixa depois do fim");
        assert_eq!(nearest_missing(0, 0, 3, |b| b == 0), None);
        assert_eq!(nearest_missing(0, 5, 0, none), Some(0));
    }

    #[test]
    fn bucket_groups_by_step() {
        assert_eq!(bucket(0.), 0);
        assert_eq!(bucket(9.99), 0);
        assert_eq!(bucket(10.), 1);
        assert_eq!(bucket(-3.), 0);
    }

    #[test]
    fn prefetches_neighbours_when_idle_and_stays_inside_the_file() {
        let dir = std::env::temp_dir().join(format!("higanbana-thumbs-prefetch-{}", std::process::id()));
        // 25 s de vídeo: faixas 0, 1 e 2.
        let (mut thumbs, mut rx) = Thumbs::open("av://lavfi:testsrc=duration=25:size=640x360:rate=24", dir).unwrap();
        thumbs.set_len(25.);
        thumbs.want(1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while (0..=2).any(|b| thumbs.get(b).is_none()) {
            assert!(std::time::Instant::now() < deadline, "vizinhas não foram geradas");
            let e = futures::executor::block_on(rx.next()).unwrap();
            if let Some(shot) = thumbs.on_event(e) {
                let ok = shot.run();
                thumbs.finish(shot.bucket, ok);
            }
        }
        assert!(thumbs.get(3).is_none(), "não gera faixa depois do fim");
    }

    /// Medição manual: `HIGANBANA_BENCH_FILE=arquivo cargo test -p higanbana-desktop bench_thumbs -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_thumbs() {
        let Ok(file) = std::env::var("HIGANBANA_BENCH_FILE") else { return };
        let variants: Vec<(&str, Vec<(&'static str, String)>)> = vec![
            ("base", vec![]),
            ("fast", vec![
                ("vd-lavc-threads", "1".into()),
                ("vd-lavc-skiploopfilter", "all".into()),
                ("vd-lavc-fast", "yes".into()),
                ("demuxer-lavf-probesize", "262144".into()),
                ("demuxer-lavf-analyzeduration", "0.3".into()),
                ("demuxer-thread", "no".into()),
            ]),
        ];
        for (name, extra) in variants {
            let dir = std::env::temp_dir().join(format!("higanbana-bench-{name}-{}", std::process::id()));
            let t0 = std::time::Instant::now();
            let (mut thumbs, mut rx) = Thumbs::open_with(&file, dir, &extra).unwrap();
            let mut open = None;
            let mut lat = Vec::new();
            for (i, b) in [20u32, 90, 35, 100, 5, 60, 77, 12].into_iter().enumerate() {
                let mut asked: Option<std::time::Instant> = None;
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
                loop {
                    if asked.is_none() && thumbs.ready {
                        asked = Some(std::time::Instant::now());
                        thumbs.want(b);
                    }
                    if thumbs.get(b).is_some() {
                        break;
                    }
                    assert!(std::time::Instant::now() < deadline, "{name}: sem miniatura {b}");
                    let e = futures::executor::block_on(rx.next()).unwrap();
                    let was_ready = thumbs.ready;
                    let shot = thumbs.on_event(e);
                    if !was_ready && thumbs.ready && open.is_none() {
                        open = Some(t0.elapsed());
                    }
                    if let Some(shot) = shot {
                        let ok = shot.run();
                        thumbs.finish(shot.bucket, ok);
                    }
                }
                let _ = i;
                lat.push(asked.map_or(0, |a| a.elapsed().as_millis()));
            }
            eprintln!("BENCH {name}: abrir={:?} miniaturas_ms={lat:?}", open);
        }
    }

    /// Gera miniaturas de um vídeo sintético (lavfi). Precisa da libmpv em `lib/`.
    #[test]
    fn makes_small_frames_for_requested_buckets() {
        let dir = std::env::temp_dir().join(format!("higanbana-thumbs-test-{}", std::process::id()));
        let (mut thumbs, mut rx) = Thumbs::open("av://lavfi:testsrc=duration=120:size=1280x720:rate=24", dir.clone()).unwrap();
        thumbs.set_len(120.);
        thumbs.want(3);
        thumbs.want(6); // substitui o 3 antes de começar
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while thumbs.get(6).is_none() {
            assert!(std::time::Instant::now() < deadline, "miniatura não saiu");
            let e = futures::executor::block_on(rx.next()).unwrap();
            if let Some(shot) = thumbs.on_event(e) {
                let ok = shot.run();
                thumbs.finish(shot.bucket, ok);
            }
        }
        assert!(thumbs.get(3).is_none(), "pedido substituído não deveria ter sido gerado");
        let path = thumbs.get(6).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8], "JPEG");
        // Largura gravada no SOF0 do JPEG: 320.
        let sof = bytes.windows(2).position(|w| w == [0xFF, 0xC0]).expect("SOF0");
        let width = u16::from_be_bytes([bytes[sof + 7], bytes[sof + 8]]);
        assert_eq!(width, WIDTH as u16);
        drop(thumbs);
        assert!(!dir.exists(), "pasta apagada ao descartar");
    }
}
