//! Seções do grupo sobreposição: Dialog, Tabs, Select, Tooltip e Toast.
use super::{preview, row};
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

const GENRES: [&str; 6] = ["Ação", "Fantasia", "Mistério", "Romance", "Slice of life", "Terror"];

/// O diálogo em si, montado na raiz para cobrir a janela toda.
pub fn dialog(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let name = Input::new("dlg-nome", &v.name, |v: &mut Gallery| &mut v.name).render(cx);
    let cancel = Button::new("dlg-cancelar", "Cancelar").variant(Variant::Outline).on_click(close).render(cx);
    let save = Button::new("dlg-salvar", "Salvar alterações")
        .on_click(|v: &mut Gallery, _, cx| {
            v.dialog = false;
            v.toasts.push(Toast::new("Perfil salvo").description(format!("Nome: {}", v.name.text())));
            cx.notify();
        })
        .render(cx);
    Dialog::new("dialogo", v.dialog)
        .title("Editar perfil")
        .description("Altere seu nome de aventureira. Clique em salvar quando terminar.")
        .width(440.0)
        .child(div().flex_col().gap(6.0).child(text("Nome").text_size(14.0).font_weight(500)).child(name))
        .footer(div().gap(8.0).child(cancel).child(save))
        .on_open_change(|v: &mut Gallery, open, cx| {
            v.dialog = open;
            cx.notify();
        })
        .render(cx)
}

fn close(v: &mut Gallery, _: &MouseEvent, cx: &mut EventCx) {
    v.dialog = false;
    cx.notify();
}

pub fn dialog_section(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let open = Button::new("abrir-dialogo", "Editar perfil")
        .variant(Variant::Outline)
        .on_click(|v: &mut Gallery, _, cx| {
            v.dialog = true;
            Dialog::<Gallery>::open(cx, "dialogo");
        })
        .render(cx);
    vec![preview(cx, "Modal", open)]
}

pub fn tabs(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let muted = cx.theme().muted_foreground;
    let panel = |title: &'static str, body: &'static str| {
        div().flex_col().gap(6.0).pt(12.0).child(text(title).font_weight(600)).child(text(body).text_size(14.0).text_color(muted))
    };
    let tabs = Tabs::new("abas", v.tab, |v: &mut Gallery, i, cx| {
        v.tab = i;
        cx.notify();
    })
    .tab("Sinopse", panel("Frieren", "Depois de derrotar o Rei Demônio, a maga elfa revisita o caminho que trilhou com o grupo do herói."))
    .tab("Episódios", panel("28 episódios", "Temporada 1 completa, com legendas em português."))
    .tab("Elenco", panel("Dublagem", "Atsumi Tanezaki, Kana Ichinose, Chiaki Kobayashi."))
    .render(cx);
    vec![preview(cx, "Abas", div().w(440.0).flex_col().child(tabs))]
}

pub fn select(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let muted = cx.theme().muted_foreground;
    let select = Select::new("genero", &v.genre, |v: &mut Gallery| &mut v.genre)
        .items(GENRES)
        .placeholder("Escolha um gênero")
        .width(220.0)
        .render(cx);
    let chosen = v.genre.selected.map_or("nenhum".to_string(), |i| GENRES[i].to_string());
    let body = div().flex_col().gap(10.0).items_center().child(select).child(text(format!("Escolhido: {chosen}")).text_size(13.0).text_color(muted));
    vec![preview(cx, "Gênero favorito", body)]
}

pub fn tooltip(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let top = Button::new("tip-btn", "Passe o mouse").variant(Variant::Outline).render(cx);
    let top = Tooltip::new("tip", &v.tip, |v: &mut Gallery| &mut v.tip, top, "Adicionar à lista").render(cx);
    let right = Button::new("tip-btn2", "À direita").variant(Variant::Secondary).render(cx);
    let right = Tooltip::new("tip2", &v.tip_side, |v: &mut Gallery| &mut v.tip_side, right, "Dica do lado direito").side(Side::Right).render(cx);
    vec![preview(cx, "Em cima e à direita", row().gap(48.0).child(top).child(right))]
}

pub fn toast(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let ok = Button::new("toast-ok", "Mostrar aviso")
        .variant(Variant::Outline)
        .on_click(|v: &mut Gallery, _, cx| {
            v.toasts.push(Toast::new("Episódio adicionado").description("Frieren · Ep. 12 entrou na sua lista."));
            cx.notify();
        })
        .render(cx);
    let err = Button::new("toast-erro", "Mostrar erro")
        .variant(Variant::Destructive)
        .on_click(|v: &mut Gallery, _, cx| {
            v.toasts.push(Toast::new("Falha no download").description("A conexão caiu. Tente de novo.").variant(Variant::Destructive));
            cx.notify();
        })
        .render(cx);
    let kit_ok = Button::new("nota-ok", "Notificar sucesso")
        .variant(Variant::Outline)
        .on_click(|v: &mut Gallery, _, cx| {
            v.notes.push_success("Download concluído");
            cx.notify();
        })
        .render(cx);
    let kit_err = Button::new("nota-erro", "Notificar erro")
        .variant(Variant::Outline)
        .on_click(|v: &mut Gallery, _, cx| {
            v.notes.push_error("Não foi possível baixar o episódio");
            cx.notify();
        })
        .render(cx);
    vec![
        preview(cx, "Padrão e destrutivo", row().child(ok).child(err)),
        preview(cx, "Notificação (topo, com ícone)", row().child(kit_ok).child(kit_err)),
    ]
}

/// Dois avisos para a captura `HGUI_GALLERY_STATE=toast`.
pub fn push_demo_toasts(t: &mut Toasts, notes: &mut Toasts) {
    notes.push_success("Download concluído");
    notes.push_error("Não foi possível baixar o episódio");
    t.push(Toast::new("Episódio adicionado").description("Frieren · Ep. 12 entrou na sua lista."));
    t.push(Toast::new("Falha no download").description("A conexão caiu. Tente de novo.").variant(Variant::Destructive));
}
