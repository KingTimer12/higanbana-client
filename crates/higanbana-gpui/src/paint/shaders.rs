//! Cache de shaders na GPU: um `GpuShader` por (tradução, backdrop) em uso, como o `ImageCache`.
//! Vazio (nenhum shader na tela) não custa nada por quadro.
use crate::gfx::{Context, GpuShader, Shader, ShaderError};
use crate::tree::Tree;
use std::collections::HashMap;

struct Entry<'c> {
    shader: Shader,
    /// `None` = `hg_shader_create` falhou (o erro já está no `Shader`).
    gpu: Option<GpuShader<'c>>,
}

pub struct ShaderCache<'c> {
    ctx: &'c Context,
    map: HashMap<(usize, bool), Entry<'c>>,
}

impl<'c> ShaderCache<'c> {
    pub fn new(ctx: &'c Context) -> Self {
        Self { ctx, map: HashMap::new() }
    }

    /// Depois de reconstruir a árvore: solta shaders órfãos e cria os novos dela.
    pub fn prepare<V>(&mut self, tree: &Tree<V>) {
        self.map.retain(|_, e| e.shader.holders() > 1);
        for n in &tree.nodes {
            if let Some(fill) = &n.style.shader {
                self.add(&fill.shader);
            }
        }
    }

    /// Cria o `GpuShader` de `s` se ainda não existe (shaders fora da árvore: hover, canvas direto).
    /// `true` = entrada nova.
    pub fn add(&mut self, s: &Shader) -> bool {
        if self.map.contains_key(&s.key()) {
            return false;
        }
        let gpu = match self.ctx.shader(s) {
            Ok(g) => Some(g),
            Err(e) => {
                report(s, ShaderError::Native(e.to_string()));
                None
            }
        };
        self.map.insert(s.key(), Entry { shader: s.clone(), gpu });
        true
    }

    /// Depois de uma troca de backend bem-sucedida: erros valem por backend. Limpa os erros
    /// e recria as entradas cuja criação falhou (o core recompila as outras sob demanda).
    pub fn reset_errors(&mut self) {
        let ctx = self.ctx;
        for e in self.map.values_mut() {
            e.shader.clear_error();
            if e.gpu.is_none() {
                e.gpu = ctx.shader(&e.shader).map_err(|err| report(&e.shader, ShaderError::Native(err.to_string()))).ok();
            }
        }
    }

    /// Gancho de teste: entrada cuja criação falhou com `e`.
    #[cfg(test)]
    pub(crate) fn insert_failed(&mut self, s: &Shader, e: ShaderError) {
        s.set_error(e);
        self.map.insert(s.key(), Entry { shader: s.clone(), gpu: None });
    }

    /// `None` = não preparado ou falhou na criação.
    pub fn get(&self, s: &Shader) -> Option<&GpuShader<'c>> {
        self.map.get(&s.key()).and_then(|e| e.gpu.as_ref())
    }

    /// Depois do `present`: o compilador nativo só roda no 1º desenho. Passa os erros novos
    /// para o `Shader` (vira magenta). `true` = há erro novo e vale repintar.
    pub fn collect_errors(&self) -> bool {
        let mut fresh = false;
        for e in self.map.values().filter(|e| !e.shader.has_error()) {
            if let Some(msg) = e.gpu.as_ref().and_then(|g| g.native_error()) {
                fresh |= report(&e.shader, ShaderError::Native(msg));
            }
        }
        fresh
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Guarda o erro no `Shader` e avisa uma vez só por erro.
fn report(s: &Shader, e: ShaderError) -> bool {
    let fresh = s.set_error(e.clone());
    if fresh {
        eprintln!("higanbana-gpui: shader: {e}");
    }
    fresh
}
