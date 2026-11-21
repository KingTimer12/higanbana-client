//! Seções do grupo formulário: Input, Checkbox, Switch e Slider.
use super::preview;
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

/// Rótulo acima do campo, com dica opcional abaixo.
fn field(label: &'static str, control: Element<Gallery>, hint: Option<(&'static str, Color)>) -> Element<Gallery> {
    let hint = hint.map(|(t, c)| text(t).text_size(13.0).text_color(c));
    div().flex_col().gap(6.0).w(320.0).child(text(label).text_size(14.0).font_weight(500)).child(control).children(hint)
}

pub fn input(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let theme = cx.theme();
    let (muted, red) = (theme.muted_foreground, theme.destructive);
    let name = Input::new("in-nome", &v.name, |v: &mut Gallery| &mut v.name).placeholder("Seu nome").render(cx);
    let email = Input::new("in-email", &v.email, |v: &mut Gallery| &mut v.email).render(cx);
    let pass = Input::new("in-senha", &v.password, |v: &mut Gallery| &mut v.password).password().render(cx);
    let invalid = Input::new("in-invalido", &v.invalid, |v: &mut Gallery| &mut v.invalid).invalid(true).render(cx);
    let off = Input::new("in-off", &v.disabled_input, |v: &mut Gallery| &mut v.disabled_input).disabled(true).render(cx);
    let grid = |a, b| div().gap(24.0).flex_wrap().justify_center().child(a).child(b);
    vec![
        preview(cx, "Normal", grid(field("Nome", name, None), field("E-mail", email, Some(("Nunca compartilhamos seu e-mail.", muted))))),
        preview(cx, "Senha e inválido", grid(field("Senha", pass, None), field("E-mail", invalid, Some(("Informe um e-mail válido.", red))))),
        preview(cx, "Desabilitado", field("Usuário", off, None)),
    ]
}

pub fn checkbox(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let terms = Checkbox::new("cb-termos", v.terms)
        .label("Aceito os termos e condições")
        .on_change(|v: &mut Gallery, on, cx| {
            v.terms = on;
            cx.notify();
        })
        .render(cx);
    let news = Checkbox::new("cb-news", v.news)
        .label("Receber novidades (indeterminado)")
        .on_change(|v: &mut Gallery, on, cx| {
            v.news = on.into();
            cx.notify();
        })
        .render(cx);
    let off = Checkbox::new("cb-off", true).label("Marcado e desabilitado").disabled(true).render(cx);
    let off2 = Checkbox::new("cb-off2", false).label("Desmarcado e desabilitado").disabled(true).render(cx);
    let col = div().flex_col().gap(14.0).child(terms).child(news).child(off).child(off2);
    vec![preview(cx, "Estados", col)]
}

pub fn switch(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let wifi = Switch::new("sw-wifi", v.wifi)
        .on_change(|v: &mut Gallery, on, cx| {
            v.wifi = on;
            cx.notify();
        })
        .render(cx);
    let plane = Switch::new("sw-aviao", v.airplane)
        .on_change(|v: &mut Gallery, on, cx| {
            v.airplane = on;
            cx.notify();
        })
        .render(cx);
    let off = Switch::new("sw-off", true).disabled(true).render(cx);
    let item = |s: Element<Gallery>, label: String| div().gap(10.0).items_center().child(s).child(text(label).text_size(14.0));
    let col = div()
        .flex_col()
        .gap(14.0)
        .child(item(wifi, format!("Wi-Fi ({})", if v.wifi { "ligado" } else { "desligado" })))
        .child(item(plane, format!("Modo avião ({})", if v.airplane { "ligado" } else { "desligado" })))
        .child(item(off, "Desabilitado".into()));
    vec![preview(cx, "Configurações", col)]
}

pub fn slider(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let muted = cx.theme().muted_foreground;
    let volume = Slider::new("sl-volume", v.volume)
        .range(0.0, 100.0)
        .step(1.0)
        .width(300.0)
        .on_change(|v: &mut Gallery, x, cx| {
            v.volume = x;
            cx.notify();
        })
        .render(cx);
    let opacity = Slider::new("sl-opac", v.opacity)
        .range(0.0, 1.0)
        .step(0.05)
        .width(300.0)
        .on_change(|v: &mut Gallery, x, cx| {
            v.opacity = x;
            cx.notify();
        })
        .render(cx);
    let off = Slider::new("sl-off", 60.0).range(0.0, 100.0).width(300.0).disabled(true).render(cx);
    let line = |label: &'static str, s: Element<Gallery>, value: String| {
        div()
            .flex_col()
            .gap(10.0)
            .child(div().w(300.0).justify_between().child(text(label).text_size(14.0).font_weight(500)).child(text(value).text_size(14.0).text_color(muted)))
            .child(s)
    };
    let col = div()
        .flex_col()
        .gap(24.0)
        .child(line("Volume", volume, format!("{:.0}%", v.volume)))
        .child(line("Opacidade", opacity, format!("{:.2}", v.opacity)))
        .child(line("Desabilitado", off, "60%".into()));
    vec![preview(cx, "Com valor", col)]
}
