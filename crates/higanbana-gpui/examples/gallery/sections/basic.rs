//! Seções do grupo básico: Button, Badge, Card, Separator, Skeleton e Avatar.
use super::{preview, row};
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

const VARIANTS: [(Variant, &str); 6] = [
    (Variant::Default, "Padrão"),
    (Variant::Secondary, "Secundário"),
    (Variant::Outline, "Contorno"),
    (Variant::Ghost, "Fantasma"),
    (Variant::Destructive, "Destrutivo"),
    (Variant::Link, "Link"),
];

pub fn button(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let mut variants = row();
    for (i, (variant, label)) in VARIANTS.iter().enumerate() {
        variants = variants.child(Button::new(("btn-var", i), *label).variant(*variant).on_click(count).render(cx));
    }
    let mut sizes = row();
    for (i, (size, label)) in [(ComponentSize::Sm, "Pequeno"), (ComponentSize::Md, "Médio"), (ComponentSize::Lg, "Grande")].into_iter().enumerate() {
        sizes = sizes.child(Button::new(("btn-tam", i), label).size(size).on_click(count).render(cx));
    }
    sizes = sizes.child(Button::new("btn-icone-so", "").size(ComponentSize::Icon).variant(Variant::Outline).icon(text("♥")).render(cx));
    let disabled = row()
        .child(Button::new("btn-off-1", "Desabilitado").disabled(true).render(cx))
        .child(Button::new("btn-off-2", "Contorno desabilitado").variant(Variant::Outline).disabled(true).render(cx));
    let icon = row()
        .child(Button::new("btn-ico-1", "Baixar episódio").icon(text("↓")).on_click(count).render(cx))
        .child(Button::new("btn-ico-2", "Favoritar").variant(Variant::Secondary).icon(text("★")).on_click(count).render(cx))
        .child(text(format!("cliques: {}", v.clicks)).text_size(13.0).text_color(cx.theme().muted_foreground));
    vec![
        preview(cx, "Variantes", variants),
        preview(cx, "Tamanhos", sizes),
        preview(cx, "Desabilitado", disabled),
        preview(cx, "Com ícone", icon),
    ]
}

fn count(v: &mut Gallery, _: &MouseEvent, cx: &mut EventCx) {
    v.clicks += 1;
    cx.notify();
}

pub fn badge(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let mut variants = row();
    for (variant, label) in VARIANTS.iter().take(5) {
        variants = variants.child(Badge::new(*label).variant(*variant).render(cx));
    }
    let mut genres = row();
    for g in ["Fantasia", "Seinen", "Slice of life", "Mistério", "Comédia"] {
        genres = genres.child(Badge::new(g).variant(Variant::Outline).render(cx));
    }
    vec![preview(cx, "Variantes", variants), preview(cx, "Gêneros", genres)]
}

pub fn kbd(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let mut keys = row();
    for k in ["ctrl-k", "alt-left", "shift-enter", "escape"] {
        keys = keys.child(Kbd::new(k).render(cx));
    }
    let outline = row().child(Kbd::new("ctrl-k").outline().render(cx));
    vec![preview(cx, "Atalhos", keys), preview(cx, "Contorno", outline)]
}

pub fn card(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let name = Input::new("card-nome", &v.name, |v: &mut Gallery| &mut v.name).render(cx);
    let email = Input::new("card-email", &v.email, |v: &mut Gallery| &mut v.email).render(cx);
    let terms = Checkbox::new("card-termos", v.terms)
        .label("Aceito os termos da guilda")
        .on_change(|v: &mut Gallery, on, cx| {
            v.terms = on;
            cx.notify();
        })
        .render(cx);
    let label = |t: &'static str| text(t).text_size(14.0).font_weight(500);
    let form = div()
        .flex_col()
        .gap(14.0)
        .child(div().flex_col().gap(6.0).child(label("Nome")).child(name))
        .child(div().flex_col().gap(6.0).child(label("E-mail")).child(email))
        .child(terms);
    let footer = div()
        .flex_1()
        .justify_between()
        .child(Button::new("card-cancelar", "Cancelar").variant(Variant::Outline).render(cx))
        .child(Button::new("card-criar", "Criar conta").disabled(!v.terms).render(cx));
    let card = Card::new()
        .header(CardHeader::new("Entrar na guilda").description("Crie sua conta de aventureiro em poucos segundos."))
        .content(form)
        .footer(footer)
        .render(cx);
    vec![preview(cx, "Card com formulário", div().w(380.0).flex_col().child(card))]
}

pub fn separator(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let h = Separator::horizontal().render(cx);
    let (v1, v2) = (Separator::vertical().render(cx), Separator::vertical().render(cx));
    let body = div()
        .w(320.0)
        .flex_col()
        .gap(12.0)
        .child(text("Higanbana UI").font_weight(600))
        .child(text("Componentes nativos acelerados por GPU.").text_size(14.0).text_color(muted))
        .child(h)
        .child(div().h(20.0).gap(12.0).items_center().text_size(14.0).child("Blog").child(v1).child("Docs").child(v2).child("Código"));
    vec![preview(cx, "Horizontal e vertical", body)]
}

pub fn skeleton(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let avatar = Skeleton::new("sk-avatar").w(48.0).h(48.0).rounded_full().render(cx);
    let l1 = Skeleton::new("sk-l1").w(250.0).h(16.0).render(cx);
    let l2 = Skeleton::new("sk-l2").w(200.0).h(16.0).render(cx);
    let profile = div().gap(16.0).items_center().child(avatar).child(div().flex_col().gap(8.0).child(l1).child(l2));
    let cover = Skeleton::new("sk-capa").w(240.0).h(130.0).render(cx);
    let t1 = Skeleton::new("sk-t1").w(240.0).h(16.0).render(cx);
    let t2 = Skeleton::new("sk-t2").w(160.0).h(16.0).render(cx);
    let card = div().flex_col().gap(10.0).child(cover).child(t1).child(t2);
    vec![preview(cx, "Perfil", profile), preview(cx, "Card", card)]
}

pub fn avatar(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let mut imgs = row();
    for size in [ComponentSize::Sm, ComponentSize::Md, ComponentSize::Lg] {
        imgs = imgs.child(Avatar::new("Frieren").image(v.cover.clone()).size(size).render(cx));
    }
    let mut fallback = row();
    for name in ["Frieren", "Fern Stark", "Himmel o Herói", "Ünlü", "葬送"] {
        fallback = fallback.child(Avatar::new(name).render(cx));
    }
    vec![preview(cx, "Com imagem (tamanhos)", imgs), preview(cx, "Iniciais como alternativa", fallback)]
}

pub fn line_clamp(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    const SINOPSE: &str = "Dois estudantes descobrem um portal escondido na biblioteca da escola e passam a viajar entre mundos, enfrentando criaturas antigas enquanto tentam voltar a tempo para as provas finais do semestre.";
    let bloco = |n: u32| div().w(280.0).flex_col().line_clamp(n).child(text(SINOPSE));
    vec![preview(cx, "1 linha", bloco(1)), preview(cx, "2 linhas", bloco(2)), preview(cx, "3 linhas", bloco(3))]
}

pub fn shapes(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let primary = cx.theme().primary;
    let tri = div().w(120.0).h(72.0).rounded(8.0).bg(cx.theme().muted).child(polygon(vec![(0.6, 0.0), (1.0, 0.0), (1.0, 1.0)], primary).absolute().top(0.0).left(0.0).size_full());
    // Xadrez 8×8 gerado na hora: mostra o recorte do zoom.
    let px: Vec<u8> = (0..64u32 * 64).flat_map(|i| if ((i % 64) / 8 + (i / 64) / 8) % 2 == 0 { [235, 70, 90, 255] } else { [40, 40, 48, 255] }).collect();
    let src = ImageData::from_rgba8(64, 64, px);
    let foto = |k: f32| img(src.clone()).w(96.0).h(96.0).rounded(12.0).object_fit(higanbana_gpui::ObjectFit::Cover).zoom(k);
    vec![preview(cx, "Polígono", tri), preview(cx, "Imagem com zoom 1× e 1,5×", row().child(foto(1.0)).child(foto(1.5)))]
}

pub fn virtual_list(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    thread_local! {
        static ALTURAS: std::sync::Arc<[f32]> = (0..10_000).map(|i| if i % 7 == 0 { 56.0 } else { 36.0 }).collect::<Vec<_>>().into();
    }
    let muted = cx.theme().muted_foreground;
    let heights = ALTURAS.with(|h| h.clone());
    let lista = VirtualList::new("galeria-lista", heights.clone()).gap(2.0).render(cx, |i, _| {
        div().h(heights[i]).px(12.0).items_center().rounded(6.0).bg(rgba(0x8080_8014)).child(text(format!("Item {i}"))).child(div().ml(8.0).text_color(muted).child(text(if i % 7 == 0 { "alto" } else { "" })))
    });
    vec![preview(cx, "10.000 itens, só os visíveis construídos", div().w(320.0).h(280.0).child(lista))]
}

pub fn carousel(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let cores = [0xe04f5f, 0x4f8fe0, 0x4fe0a0, 0xe0c04f];
    let slides = (0..4).map(|i| div().h(140.0).rounded(10.0).bg(rgb(cores[i])).items_center().justify_center().child(text(format!("Destaque {}", i + 1)))).collect();
    let banner = Carousel::new("galeria-banner", &v.banner, |g: &mut Gallery| &mut g.banner).items(slides).render(cx);
    let cards = (0..12).map(|i| div().h(96.0).rounded(8.0).bg(rgba(0x8080_8030)).items_center().justify_center().child(text(format!("{}", i + 1)))).collect();
    let fileira = Carousel::new("galeria-fileira", &v.fileira, |g: &mut Gallery| &mut g.fileira).items(cards).item_width(72.0).gap(8.0).controls(true).render(cx);
    vec![preview(cx, "Banner com auto-avanço (4 s)", div().w(420.0).child(banner)), preview(cx, "Fileira com setas", div().w(420.0).px(40.0).child(fileira))]
}
