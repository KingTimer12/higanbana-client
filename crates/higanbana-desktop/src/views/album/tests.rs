//! Encarte: quando vira álbum, discos e soltura ao sair da pasta.
use std::rc::Rc;

use higanbana_core::music::album::{Album, Track};

use super::{body_for, disc_groups, Body};
use crate::state::album::{keep_album, AlbumLoad, AlbumSlot};

fn track(disc: u32, n: u32) -> Track {
    Track { path: format!("a/{disc}-{n}.flac"), size: 1, name: format!("{n}.flac"), disc, number: Some(n), title: format!("{n}"), ext: "flac".into() }
}

fn album(tracks: Vec<Track>, discs: Vec<u32>) -> Album {
    Album { path: "a".into(), title: "A".into(), catalog: None, year: None, tracks, discs, cover_path: None, cover_size: None, others: vec![], disc_dirs: vec![] }
}

#[test]
fn nao_album_vira_lista() {
    assert_eq!(body_for(false, None), Body::Files, "sem cara de álbum: lista");
    assert_eq!(body_for(true, Some(&AlbumLoad::NotAlbum)), Body::Files, "nada tocável: lista, sem tela vazia");
    assert_eq!(body_for(true, Some(&AlbumLoad::Loading)), Body::Loading);
    assert_eq!(body_for(true, Some(&AlbumLoad::Ready(Rc::new(album(vec![track(1, 1)], vec![1]))))), Body::Album);
}

#[test]
fn faixas_agrupadas_por_disco() {
    let a = album(vec![track(1, 1), track(1, 2), track(2, 1)], vec![1, 2]);
    assert_eq!(disc_groups(&a), vec![(1, vec![0, 1]), (2, vec![2])]);
}

#[test]
fn album_solto_ao_sair() {
    let mut slot = Some(AlbumSlot { path: "OSTs/X".into(), load: AlbumLoad::Loading, files_tab: false });
    keep_album(&mut slot, "OSTs/X");
    assert!(slot.is_some(), "mesma pasta fica");
    keep_album(&mut slot, "OSTs");
    assert!(slot.is_none(), "outra tela solta");
}
