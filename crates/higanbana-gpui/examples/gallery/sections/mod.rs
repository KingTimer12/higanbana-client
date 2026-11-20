//! Seções da galeria: tabela com título/grupo/descrição e a moldura de cada página.
pub mod basic;
pub mod form;
pub mod overlay;

use crate::Gallery;
use higanbana_gpui::prelude::*;

/// Uma página da galeria.
pub struct Section {
    pub group: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub build: fn(&Gallery, &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>>,
}

const fn s(group: &'static str, title: &'static str, description: &'static str, build: fn(&Gallery, &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>>) -> Section {
    Section { group, title, description, build }
}

pub const SECTIONS: &[Section] = &[
    s("Básicos", "Button", "Dispara uma ação. Seis variantes, quatro tamanhos, ícone e estado desabilitado.", basic::button),
    s("Básicos", "Badge", "Etiqueta curta para status, gênero ou contagem.", basic::badge),
    s("Básicos", "Card", "Agrupa conteúdo relacionado com cabeçalho, corpo e rodapé.", basic::card),
    s("Básicos", "Separator", "Linha fina que divide conteúdo na horizontal ou vertical.", basic::separator),
    s("Básicos", "Skeleton", "Placeholder pulsante enquanto o conteúdo carrega.", basic::skeleton),
    s("Básicos", "Avatar", "Imagem do usuário com iniciais como alternativa.", basic::avatar),
    s("Formulário", "Input", "Campo de texto de uma linha, com senha, inválido e desabilitado.", form::input),
    s("Formulário", "Checkbox", "Marca uma opção; aceita estado indeterminado.", form::checkbox),
    s("Formulário", "Switch", "Liga e desliga uma configuração na hora.", form::switch),
    s("Formulário", "Slider", "Escolhe um valor num intervalo arrastando ou com as setas.", form::slider),
    s("Sobreposição", "Dialog", "Janela modal sobre a página; fecha com Escape ou clique fora.", overlay::dialog_section),
    s("Sobreposição", "Tabs", "Alterna entre painéis de conteúdo relacionados.", overlay::tabs),
    s("Sobreposição", "Select", "Lista suspensa para escolher uma opção.", overlay::select),
    s("Sobreposição", "Tooltip", "Dica curta que aparece ao parar o mouse em cima.", overlay::tooltip),
    s("Sobreposição", "Toast", "Aviso temporário no canto da tela.", overlay::toast),
];

/// Título, descrição e os exemplos da seção ativa.
pub fn page(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let sec = &SECTIONS[v.section.min(SECTIONS.len() - 1)];
    let demos = (sec.build)(v, cx);
    let theme = cx.theme();
    div()
        .flex_col()
        .gap(20.0)
        .max_w(860.0)
        .child(text(sec.group).text_size(13.0).text_color(theme.primary).font_weight(600))
        .child(text(sec.title).text_size(30.0).font_bold())
        .child(text(sec.description).text_size(16.0).text_color(theme.muted_foreground))
        .children(demos)
}

/// Caixa de exemplo no estilo da doc do shadcn: rótulo + área centralizada com borda.
pub fn preview(cx: &RenderCx<'_, Gallery>, label: &'static str, body: Element<Gallery>) -> Element<Gallery> {
    let theme = cx.theme();
    div()
        .flex_col()
        .gap(8.0)
        .flex_shrink_0()
        .child(text(label).text_size(14.0).font_weight(600))
        .child(
            div()
                .min_h(140.0)
                .p(32.0)
                .rounded(theme.radius)
                .border(1.0, theme.border)
                .items_center()
                .justify_center()
                .child(body),
        )
}

/// Linha com quebra para enfileirar variantes.
pub fn row() -> Element<Gallery> {
    div().flex_wrap().gap(12.0).items_center().justify_center()
}
