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

fn options() -> Vec<(&'static str, String)> {
    vec![
        ("vo", "null".into()),
        ("pause", "yes".into()),
        ("keep-open", "yes".into()),
        ("audio", "no".into()),
        ("sid", "no".into()),
        ("hwdec", "no".into()),
        ("hr-seek", "no".into()),
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
    done: HashMap<u32, Arc<Path>>,
}

impl Thumbs {
    /// Abre `url` numa instância sem janela; as miniaturas vão para `dir`.
    pub fn open(url: &str, dir: PathBuf) -> Result<(Self, UnboundedReceiver<MpvEvent>), String> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let (mpv, rx) = Mpv::new(&options(), &[])?;
        mpv.command(&["loadfile", url])?;
        Ok((Self { mpv: Arc::new(mpv), dir, ready: false, busy: None, want: None, done: HashMap::new() }, rx))
    }

    /// Miniatura pronta da faixa.
    pub fn get(&self, bucket: u32) -> Option<Arc<Path>> {
        self.done.get(&bucket).cloned()
    }

    /// Pede a faixa (substitui o pedido anterior que ainda não começou).
    pub fn want(&mut self, bucket: u32) {
        self.want = Some(bucket);
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
        let Some(b) = self.want.filter(|b| self.ready && self.busy.is_none() && !self.done.contains_key(b)) else { return };
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
    fn bucket_groups_by_step() {
        assert_eq!(bucket(0.), 0);
        assert_eq!(bucket(9.99), 0);
        assert_eq!(bucket(10.), 1);
        assert_eq!(bucket(-3.), 0);
    }

    /// Gera miniaturas de um vídeo sintético (lavfi). Precisa da libmpv em `lib/`.
    #[test]
    fn makes_small_frames_for_requested_buckets() {
        let dir = std::env::temp_dir().join(format!("higanbana-thumbs-test-{}", std::process::id()));
        let (mut thumbs, mut rx) = Thumbs::open("av://lavfi:testsrc=duration=120:size=1280x720:rate=24", dir.clone()).unwrap();
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
