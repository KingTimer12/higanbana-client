use super::api::{Node, NodeList, NodeValue, RawProperty, END_REASON_ERROR, FORMAT_DOUBLE, FORMAT_NODE, FORMAT_NODE_ARRAY, FORMAT_NODE_MAP, FORMAT_STRING};
use super::options::{PlayerOptions, DEFAULT_OPTIONS};
use super::{Player, PlayerState};
use std::ffi::{c_void, CString};

#[test]
fn opcoes_padrao_iguais_as_atuais() {
    let o = PlayerOptions::default();
    let pares: Vec<(&str, &str)> = o.pairs().map(|(k, v)| (k, v)).collect();
    assert_eq!(pares, DEFAULT_OPTIONS.to_vec());
}

#[test]
fn set_sobrescreve_chave() {
    let o = PlayerOptions::default().set("hwdec", "no").set("cache", "yes");
    let v: Vec<_> = o.pairs().filter(|(k, _)| *k == "hwdec").collect();
    assert_eq!(v, vec![("hwdec", "no")]);
    assert!(o.pairs().any(|p| p == ("cache", "yes")));
}

#[test]
fn load_at_sem_start_ate_1s() {
    let p = Player::detached();
    p.load_at("a", Some(0.5));
    p.load_at("b", Some(12.34));
    p.load_at("c", None);
    assert_eq!(p.commands(), vec!["loadfile a replace -1", "loadfile b replace -1 start=12.3", "loadfile c replace -1"]);
}

#[test]
fn seek_e_exato() {
    let p = Player::detached();
    p.seek(3.0);
    assert_eq!(p.commands(), vec!["seek 3.000 absolute+exact"]);
}

fn s(t: &str) -> *mut std::ffi::c_char {
    CString::new(t).unwrap().into_raw()
}

fn chapter(time: f64, title: &str) -> Node {
    let mut values = vec![Node { u: NodeValue { string: s(title) }, format: FORMAT_STRING }, Node { u: NodeValue { double: time }, format: FORMAT_DOUBLE }];
    let mut keys = vec![s("title"), s("time")];
    let list = Box::new(NodeList { num: 2, values: values.as_mut_ptr(), keys: keys.as_mut_ptr() });
    std::mem::forget((values, keys));
    Node { u: NodeValue { list: Box::into_raw(list) }, format: FORMAT_NODE_MAP }
}

#[test]
fn capitulos_do_node() {
    let mut items = vec![chapter(0.0, "Intro"), chapter(90.5, "Parte A")];
    let list = NodeList { num: 2, values: items.as_mut_ptr(), keys: std::ptr::null_mut() };
    let mut list = Box::new(list);
    let mut node = Node { u: NodeValue { list: &mut *list }, format: FORMAT_NODE_ARRAY };
    let name = CString::new("chapter-list").unwrap();
    let raw = RawProperty { name: name.as_ptr(), format: FORMAT_NODE, data: &mut node as *mut Node as *mut c_void };
    let mut st = PlayerState::default();
    unsafe { st.apply(&raw) };
    assert_eq!(st.chapters.len(), 2);
    assert_eq!(st.chapters[1].title, "Parte A");
    assert!((st.chapters[1].time - 90.5).abs() < 1e-9);
}

#[test]
fn cache_e_buffering() {
    let mut st = PlayerState::default();
    let mut d = 12.5f64;
    let name = CString::new("demuxer-cache-time").unwrap();
    unsafe { st.apply(&RawProperty { name: name.as_ptr(), format: FORMAT_DOUBLE, data: &mut d as *mut f64 as *mut c_void }) };
    assert_eq!(st.cache_ahead, 12.5);
    let mut f: std::ffi::c_int = 1;
    for (prop, check) in [("paused-for-cache", 0), ("vo-configured", 1)] {
        let n = CString::new(prop).unwrap();
        unsafe { st.apply(&RawProperty { name: n.as_ptr(), format: super::api::FORMAT_FLAG, data: &mut f as *mut _ as *mut c_void }) };
        let _ = check;
    }
    assert!(st.buffering && st.first_frame);
}

#[test]
fn fim_com_erro_incrementa() {
    let mut st = PlayerState::default();
    st.end_file(END_REASON_ERROR, -13);
    st.end_file(0, 0);
    st.end_file(END_REASON_ERROR, -13);
    assert_eq!(st.end_error, 2);
}

#[test]
fn eof_velho_durante_carregamento_ignorado() {
    let mut st = PlayerState::default();
    st.status = super::Status::Loading;
    let mut f: std::ffi::c_int = 1;
    let n = CString::new("eof-reached").unwrap();
    unsafe { st.apply(&RawProperty { name: n.as_ptr(), format: super::api::FORMAT_FLAG, data: &mut f as *mut _ as *mut c_void }) };
    assert_eq!(st.status, super::Status::Loading);
}

#[test]
fn reinicio_de_reproducao_conta() {
    let mut st = PlayerState::default();
    st.playback_restart();
    st.playback_restart();
    assert_eq!(st.restarts, 2);
}
