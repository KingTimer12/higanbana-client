//! Histórico de navegação (voltar/avançar) por caminho do acervo.

pub struct Nav {
    stack: Vec<String>,
    index: usize,
    /// A última navegação voltou no histórico (a página nova entra pela esquerda).
    went_back: bool,
}

impl Default for Nav {
    fn default() -> Self {
        Nav { stack: vec![String::new()], index: 0, went_back: false }
    }
}

impl Nav {
    pub fn path(&self) -> &str {
        &self.stack[self.index]
    }

    /// Vai para `path`; `false` se já estava lá. Apaga o "avançar".
    pub fn go(&mut self, path: impl Into<String>) -> bool {
        let path = path.into();
        if path == self.path() {
            return false;
        }
        self.stack.truncate(self.index + 1);
        self.stack.push(path);
        self.index += 1;
        self.went_back = false;
        true
    }

    pub fn went_back(&self) -> bool {
        self.went_back
    }

    pub fn can_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.stack.len()
    }

    pub fn back(&mut self) -> bool {
        if !self.can_back() {
            return false;
        }
        self.index -= 1;
        self.went_back = true;
        true
    }

    pub fn forward(&mut self) -> bool {
        if !self.can_forward() {
            return false;
        }
        self.index += 1;
        self.went_back = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voltar_e_avancar_seguem_historico() {
        let mut n = Nav::default();
        assert_eq!(n.path(), "");
        assert!(n.go("Animes"));
        assert!(n.go("Animes/Letra A"));
        assert!(n.back());
        assert_eq!(n.path(), "Animes");
        assert!(n.forward());
        assert_eq!(n.path(), "Animes/Letra A");
        assert!(!n.forward(), "sem nada à frente");
        n.back();
        n.back();
        assert!(!n.back(), "início do histórico");
        assert_eq!(n.path(), "");
    }

    #[test]
    fn go_limpa_avancar() {
        let mut n = Nav::default();
        n.go("a");
        n.go("b");
        n.back();
        n.go("c");
        assert!(!n.can_forward());
        assert_eq!(n.path(), "c");
        assert!(!n.go("c"), "mesmo caminho não empilha");
    }

    #[test]
    fn went_back_marca_direcao() {
        let mut n = Nav::default();
        n.go("a");
        n.back();
        assert!(n.went_back());
        n.forward();
        assert!(!n.went_back());
    }
}
