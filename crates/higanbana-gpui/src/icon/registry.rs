//! Nome → bytes SVG. Começa com os Lucide embutidos; o app pode somar os seus.
use std::cell::RefCell;
use std::collections::HashMap;

/// Conjunto de ícones registrável com `App::icons`.
#[derive(Default, Clone)]
pub struct IconSet {
    pub(crate) items: Vec<(&'static str, &'static [u8])>,
}

impl IconSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// `name` é como o ícone é pedido em `icon(name)`; `svg` costuma vir de `include_bytes!`.
    pub fn add(mut self, name: &'static str, svg: &'static [u8]) -> Self {
        self.items.push((name, svg));
        self
    }

    /// Os Lucide que a lib embute (kebab-case: `"arrow-left"`, `"volume-2"`...).
    pub fn lucide() -> Self {
        IconSet { items: super::lucide::LUCIDE.to_vec() }
    }
}

thread_local! {
    static ICONS: RefCell<HashMap<&'static str, &'static [u8]>> =
        RefCell::new(super::lucide::LUCIDE.iter().copied().collect());
}

/// Soma (ou substitui) ícones no registro da thread da UI.
pub fn register(set: IconSet) {
    ICONS.with(|m| m.borrow_mut().extend(set.items));
    super::raster::clear();
}

pub(crate) fn svg(name: &str) -> Option<&'static [u8]> {
    ICONS.with(|m| m.borrow().get(name).copied())
}

/// `icon_set!["logo" => "../assets/logo.svg", ...]` embute os arquivos com `include_bytes!`.
#[macro_export]
macro_rules! icon_set {
    ($($name:literal => $path:literal),* $(,)?) => {
        $crate::icon::IconSet::new()$(.add($name, include_bytes!($path)))*
    };
}

/// Existe ícone com esse nome (Lucide embutido ou registrado pelo app)?
pub fn exists(name: &str) -> bool {
    svg(name).is_some()
}
