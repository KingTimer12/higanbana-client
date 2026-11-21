use higanbana_gpui_sys::*;

unsafe fn put(a: *mut hg_atlas, key: u64, w: u32, h: u32, frame: u64) -> (i32, hg_rect, i32) {
    let mut uv = hg_rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };
    let mut inserted = 0;
    let r = unsafe { hg__atlas_get_or_insert(a, key, w, h, frame, &mut uv, &mut inserted) };
    (r, uv, inserted)
}

#[test]
fn same_key_returns_same_uv_without_insert() {
    unsafe {
        let a = hg__atlas_create(256, 256);
        let (r1, uv1, ins1) = put(a, 7, 32, 32, 0);
        let (r2, uv2, ins2) = put(a, 7, 32, 32, 1);
        assert_eq!((r1, r2), (HG_OK as i32, HG_OK as i32));
        assert_eq!((ins1, ins2), (1, 0));
        assert_eq!((uv1.x, uv1.y), (uv2.x, uv2.y));
        hg__atlas_destroy(a);
    }
}

#[test]
fn full_atlas_evicts_least_recent() {
    unsafe {
        let a = hg__atlas_create(256, 256);
        for k in 0..16u64 {
            assert_eq!(put(a, k, 64, 64, k).0, HG_OK as i32);
        }
        put(a, 0, 64, 64, 16); // toca key 0
        assert_eq!(put(a, 100, 64, 64, 17).0, HG_OK as i32);
        assert_eq!(hg__atlas_contains(a, 0), 1);
        assert_eq!(hg__atlas_contains(a, 1), 0);
        assert_eq!(hg__atlas_contains(a, 100), 1);
        hg__atlas_destroy(a);
    }
}

#[test]
fn oversize_entry_is_oom() {
    unsafe {
        let a = hg__atlas_create(256, 256);
        assert_eq!(put(a, 1, 512, 512, 0).0, HG_ERR_OOM as i32);
        hg__atlas_destroy(a);
    }
}
