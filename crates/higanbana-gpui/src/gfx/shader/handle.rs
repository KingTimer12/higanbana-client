//! `Shader`: fonte WGSL do usuário já traduzido, barato de clonar.
use super::error::ShaderError;
use super::translate::Translations;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

struct Inner {
    source: String,
    translations: Translations,
    origin: Instant,
    error: RefCell<Option<ShaderError>>,
}

/// Shader do usuário: `fn shade(p: ShadeInput) -> vec4<f32>` em WGSL.
/// A tradução acontece uma vez em `wgsl`; clones dividem o mesmo resultado.
#[derive(Clone)]
pub struct Shader {
    inner: Rc<Inner>,
    backdrop: bool,
    animated: bool,
}

impl Shader {
    /// Compõe com o prelúdio da lib e traduz para todos os backends.
    pub fn wgsl(src: &str) -> Result<Shader, ShaderError> {
        let translations = translate(src)?;
        let inner = Inner { source: src.to_owned(), translations, origin: Instant::now(), error: RefCell::new(None) };
        Ok(Shader { inner: Rc::new(inner), backdrop: false, animated: false })
    }

    /// Copia o que está atrás do retângulo antes de desenhar (`backdrop()` no WGSL).
    pub fn backdrop(mut self, on: bool) -> Self {
        self.backdrop = on;
        self
    }

    /// Pede quadro contínuo enquanto o shader estiver visível.
    pub fn animated(mut self, on: bool) -> Self {
        self.animated = on;
        self
    }

    pub fn is_backdrop(&self) -> bool {
        self.backdrop
    }

    pub fn is_animated(&self) -> bool {
        self.animated
    }

    /// Último erro do compilador nativo (aparece no 1º uso em cada backend).
    pub fn last_error(&self) -> Option<ShaderError> {
        self.inner.error.borrow().clone()
    }

    /// Esquece o erro (troca de backend: o próximo backend compila de novo e avisa de novo).
    pub(crate) fn clear_error(&self) {
        self.inner.error.borrow_mut().take();
    }

    /// Tem erro guardado? (sem clonar a mensagem; usado a cada pintura)
    pub(crate) fn has_error(&self) -> bool {
        self.inner.error.borrow().is_some()
    }

    /// Segundos desde a criação (o `time` do `ShadeInput`).
    pub fn time(&self) -> f32 {
        self.inner.origin.elapsed().as_secs_f32()
    }

    /// Código WGSL do usuário, como foi passado.
    pub fn source(&self) -> &str {
        &self.inner.source
    }

    /// Fontes traduzidos para cada backend.
    pub fn translations(&self) -> &Translations {
        &self.inner.translations
    }

    /// Mesmo shader (mesma tradução), independentemente das flags.
    pub fn same(&self, other: &Shader) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }

    /// Chave do cache de GPU: mesma tradução + backdrop (a flag fica gravada no `GpuShader`).
    pub(crate) fn key(&self) -> (usize, bool) {
        (Rc::as_ptr(&self.inner) as usize, self.backdrop)
    }

    /// Quantos `Shader` dividem esta tradução (o cache solta quando só ele sobra).
    pub(crate) fn holders(&self) -> usize {
        Rc::strong_count(&self.inner)
    }

    /// Guarda o erro do compilador nativo; devolve `true` se é novo (para avisar uma vez só).
    pub(crate) fn set_error(&self, e: ShaderError) -> bool {
        let mut slot = self.inner.error.borrow_mut();
        let new = slot.as_ref() != Some(&e);
        *slot = Some(e);
        new
    }
}

impl std::fmt::Debug for Shader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shader").field("backdrop", &self.backdrop).field("animated", &self.animated).finish_non_exhaustive()
    }
}

#[cfg(feature = "shaders")]
fn translate(src: &str) -> Result<Translations, ShaderError> {
    let (full, prelude_lines) = super::compose::compose(src);
    super::translate::translate(full, super::locate::UserRegion::new(prelude_lines, src))
}

#[cfg(not(feature = "shaders"))]
fn translate(_src: &str) -> Result<Translations, ShaderError> {
    Err(ShaderError::Unsupported)
}
