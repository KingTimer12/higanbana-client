//! Leitura de `.rar`/`.cbr` por pedaços (RAR4 e RAR5), só para arquivos guardados
//! sem compressão ("store"), que é como mangás costumam ser empacotados.
//!
//! Diferente do zip, o RAR não tem índice no fim: cada arquivo vem com um
//! cabeçalho logo antes dos dados. Lemos um cabeçalho, pulamos os dados e
//! seguimos para o próximo — cada passo é um pedido pequeno com `Range`.

use thiserror::Error;

use super::zip::Entry;

pub const RAR4_SIG: &[u8] = b"Rar!\x1a\x07\x00";
pub const RAR5_SIG: &[u8] = b"Rar!\x1a\x07\x01\x00";
/// Bytes buscados por cabeçalho (cabe com folga um nome longo).
pub const HEADER_GUESS: u64 = 2048;
/// `Entry::method` para dados compactados (não dá para ler sem descompactar o RAR).
pub const METHOD_PACKED: u16 = 0xFFFF;

#[derive(Debug, Error, PartialEq)]
pub enum RarError {
    #[error("o arquivo não é um rar válido")]
    NotRar,
    #[error("o rar está corrompido ou incompleto")]
    Corrupt,
    #[error("rar com senha não é suportado")]
    Encrypted,
    #[error("rar dividido em partes não é suportado")]
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Version {
    V4,
    V5,
}

/// Versão e onde fica o primeiro cabeçalho.
pub fn detect(head: &[u8]) -> Option<(Version, u64)> {
    if head.starts_with(RAR5_SIG) {
        Some((Version::V5, RAR5_SIG.len() as u64))
    } else if head.starts_with(RAR4_SIG) {
        Some((Version::V4, RAR4_SIG.len() as u64))
    } else {
        None
    }
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}
/// Inteiro de tamanho variável do RAR5 (7 bits por byte).
fn vint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let c = *b.get(*i)?;
        *i += 1;
        v |= ((c & 0x7f) as u64) << shift;
        if c & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// Nome unicode do RAR4: parte ascii + codificação compacta em UTF-16.
fn decode_rar4_unicode(ascii: &[u8], enc: &[u8]) -> Option<String> {
    let high = *enc.first()? as u16;
    let mut out: Vec<u16> = Vec::new();
    let (mut pos, mut flags, mut bits) = (1, 0u8, 0);
    while pos < enc.len() {
        if bits == 0 {
            flags = enc[pos];
            pos += 1;
            bits = 8;
        }
        match flags >> 6 {
            0 => {
                out.push(*enc.get(pos)? as u16);
                pos += 1;
            }
            1 => {
                out.push(*enc.get(pos)? as u16 | high << 8);
                pos += 1;
            }
            2 => {
                out.push(*enc.get(pos)? as u16 | (*enc.get(pos + 1)? as u16) << 8);
                pos += 2;
            }
            _ => {
                let len = *enc.get(pos)?;
                pos += 1;
                if len & 0x80 != 0 {
                    let correction = *enc.get(pos)? as u16;
                    pos += 1;
                    for _ in 0..(len & 0x7f) as usize + 2 {
                        let ch = *ascii.get(out.len())? as u16;
                        out.push(((ch + correction) & 0xff) | high << 8);
                    }
                } else {
                    for _ in 0..len as usize + 2 {
                        out.push(*ascii.get(out.len())? as u16);
                    }
                }
            }
        }
        flags <<= 2;
        bits -= 2;
    }
    Some(String::from_utf16_lossy(&out))
}

fn file_entry(name: String, stored: bool, packed: u64, size: u64, data_offset: u64) -> Entry {
    Entry {
        name: if name.contains('\\') { name.replace('\\', "/") } else { name },
        method: if stored { 0 } else { METHOD_PACKED },
        compressed_size: packed,
        size,
        local_offset: 0,
        data_offset: Some(data_offset),
    }
}

fn parse_v4(b: &[u8], at: u64) -> Result<Result<Parsed, usize>, RarError> {
    if b.len() < 7 {
        return Ok(Err(7));
    }
    let c = || RarError::Corrupt;
    let htype = b[2];
    let flags = u16_at(b, 3).ok_or_else(c)?;
    let head_size = u16_at(b, 5).ok_or_else(c)? as usize;
    if head_size < 7 {
        return Err(RarError::Corrupt);
    }
    let long = flags & 0x8000 != 0;
    if long && b.len() < 11 {
        return Ok(Err(11));
    }
    match htype {
        0x73 => {
            if flags & 0x0080 != 0 {
                return Err(RarError::Encrypted);
            }
            Ok(Ok(Parsed { block: Block::Other, next: at + head_size as u64 }))
        }
        0x74 => {
            if b.len() < head_size {
                return Ok(Err(head_size));
            }
            if flags & 0x04 != 0 {
                return Err(RarError::Encrypted);
            }
            if flags & 0x03 != 0 {
                return Err(RarError::Split);
            }
            let mut packed = u32_at(b, 7).ok_or_else(c)? as u64;
            let mut size = u32_at(b, 11).ok_or_else(c)? as u64;
            let method = b[25];
            let name_len = u16_at(b, 26).ok_or_else(c)? as usize;
            let mut name_at = 32;
            if flags & 0x100 != 0 {
                packed |= (u32_at(b, 32).ok_or_else(c)? as u64) << 32;
                size |= (u32_at(b, 36).ok_or_else(c)? as u64) << 32;
                name_at = 40;
            }
            let raw = b.get(name_at..name_at + name_len).ok_or_else(c)?;
            let name = match (flags & 0x200 != 0, raw.iter().position(|&x| x == 0)) {
                (true, Some(z)) => decode_rar4_unicode(&raw[..z], &raw[z + 1..])
                    .unwrap_or_else(|| String::from_utf8_lossy(&raw[..z]).into_owned()),
                _ => String::from_utf8_lossy(raw).into_owned(),
            };
            let data_offset = at + head_size as u64;
            let next = data_offset + packed;
            let block = if flags & 0xE0 == 0xE0 {
                Block::Other // pasta
            } else {
                Block::File(file_entry(name, method == 0x30, packed, size, data_offset))
            };
            Ok(Ok(Parsed { block, next }))
        }
        0x7B => Ok(Ok(Parsed { block: Block::End, next: at + head_size as u64 })),
        _ => {
            let add = if long { u32_at(b, 7).ok_or_else(c)? as u64 } else { 0 };
            Ok(Ok(Parsed { block: Block::Other, next: at + head_size as u64 + add }))
        }
    }
}

fn parse_v5(b: &[u8], at: u64) -> Result<Result<Parsed, usize>, RarError> {
    let c = || RarError::Corrupt;
    let mut i = 4;
    let Some(hsize) = vint(b, &mut i) else {
        return Ok(Err(16));
    };
    let header_end = i + hsize as usize;
    if b.len() < header_end {
        return Ok(Err(header_end));
    }
    let htype = vint(b, &mut i).ok_or_else(c)?;
    let hflags = vint(b, &mut i).ok_or_else(c)?;
    let extra = if hflags & 1 != 0 { vint(b, &mut i).ok_or_else(c)? } else { 0 };
    let data = if hflags & 2 != 0 { vint(b, &mut i).ok_or_else(c)? } else { 0 };
    if hflags & 0x18 != 0 {
        return Err(RarError::Split);
    }
    let data_offset = at + header_end as u64;
    let next = data_offset + data;
    let block = match htype {
        4 => return Err(RarError::Encrypted),
        5 => Block::End,
        2 => {
            let fflags = vint(b, &mut i).ok_or_else(c)?;
            let size = vint(b, &mut i).ok_or_else(c)?;
            vint(b, &mut i).ok_or_else(c)?; // atributos
            if fflags & 2 != 0 {
                i += 4;
            }
            if fflags & 4 != 0 {
                i += 4;
            }
            let comp = vint(b, &mut i).ok_or_else(c)?;
            vint(b, &mut i).ok_or_else(c)?; // sistema
            let name_len = vint(b, &mut i).ok_or_else(c)? as usize;
            let name = String::from_utf8_lossy(b.get(i..i + name_len).ok_or_else(c)?).into_owned();
            // Registro de criptografia (tipo 1) na área extra = arquivo com senha.
            let mut x = header_end.checked_sub(extra as usize).ok_or_else(c)?;
            while x < header_end {
                let rsize = vint(b, &mut x).ok_or_else(c)? as usize;
                let start = x;
                if vint(b, &mut x).ok_or_else(c)? == 1 {
                    return Err(RarError::Encrypted);
                }
                x = start + rsize;
            }
            if fflags & 1 != 0 {
                Block::Other // pasta
            } else {
                Block::File(file_entry(name, (comp >> 7) & 7 == 0, data, size, data_offset))
            }
        }
        _ => Block::Other,
    };
    Ok(Ok(Parsed { block, next }))
}

#[derive(Debug, PartialEq)]
pub enum Block {
    File(Entry),
    End,
    Other,
}

#[derive(Debug, PartialEq)]
pub struct Parsed {
    pub block: Block,
    /// Offset do próximo cabeçalho.
    pub next: u64,
}

/// Lê o cabeçalho que começa em `buf[0]` (offset `at` no arquivo).
/// `Ok(Err(n))` = faltaram bytes: buscar pelo menos `n` a partir de `at`.
pub fn parse_block(v: Version, buf: &[u8], at: u64) -> Result<Result<Parsed, usize>, RarError> {
    match v {
        Version::V4 => parse_v4(buf, at),
        Version::V5 => parse_v5(buf, at),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // ---------- construtores de rar falsos ----------

    /// RAR4 com arquivos "store" (method 0x30). `unicode` grava o nome no formato unicode do RAR.
    pub fn build_rar4(files: &[(&str, &[u8])], unicode_name: Option<(&str, &[u8])>) -> Vec<u8> {
        let mut out = RAR4_SIG.to_vec();
        // Cabeçalho principal (0x73): crc, tipo, flags, tamanho 13, 6 bytes reservados.
        out.extend([0, 0, 0x73, 0, 0, 13, 0, 0, 0, 0, 0, 0, 0]);
        let mut push = |name_bytes: Vec<u8>, data: &[u8], flags: u16| {
            let head_size = 32 + name_bytes.len();
            out.extend([0, 0, 0x74]);
            out.extend((flags | 0x8000).to_le_bytes());
            out.extend((head_size as u16).to_le_bytes());
            out.extend((data.len() as u32).to_le_bytes()); // pack
            out.extend((data.len() as u32).to_le_bytes()); // unp
            out.push(2); // host os
            out.extend([0u8; 4]); // crc
            out.extend([0u8; 4]); // tempo
            out.push(29); // versão
            out.push(0x30); // store
            out.extend((name_bytes.len() as u16).to_le_bytes());
            out.extend([0x20, 0, 0, 0]); // atributos
            out.extend(&name_bytes);
            out.extend(data);
        };
        for (name, data) in files {
            push(name.as_bytes().to_vec(), data, 0);
        }
        if let Some((ascii, enc)) = unicode_name {
            let mut nb = ascii.as_bytes().to_vec();
            nb.push(0);
            nb.extend(enc);
            push(nb, b"u", 0x200);
        }
        // Fim (0x7B).
        out.extend([0, 0, 0x7B, 0, 0x40, 7, 0]);
        out
    }

    fn vint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return out;
            }
            out.push(b | 0x80);
        }
    }

    /// Bloco RAR5: crc + tamanho + (tipo, flags, [extra], [dados], corpo).
    fn block5(htype: u64, flags: u64, data_size: Option<u64>, body: &[u8]) -> Vec<u8> {
        let mut h = vint(htype);
        h.extend(vint(flags | if data_size.is_some() { 2 } else { 0 }));
        if let Some(d) = data_size {
            h.extend(vint(d));
        }
        h.extend(body);
        let mut out = vec![0u8; 4];
        out.extend(vint(h.len() as u64));
        out.extend(h);
        out
    }

    /// RAR5 com arquivos; `method` 0 = store.
    pub fn build_rar5(files: &[(&str, &[u8], u64)]) -> Vec<u8> {
        let mut out = RAR5_SIG.to_vec();
        out.extend(block5(1, 0, None, &vint(0)));
        for (name, data, method) in files {
            let mut body = vint(0); // file flags
            body.extend(vint(data.len() as u64));
            body.extend(vint(0x20)); // atributos
            body.extend(vint(method << 7)); // compressão
            body.extend(vint(0)); // host os
            body.extend(vint(name.len() as u64));
            body.extend(name.as_bytes());
            out.extend(block5(2, 0, Some(data.len() as u64), &body));
            out.extend(*data);
        }
        out.extend(block5(5, 0, None, &vint(0)));
        out
    }

    /// Caminha pelos cabeçalhos como o leitor faz (aqui com o arquivo inteiro na memória).
    pub fn walk(b: &[u8]) -> Result<Vec<Entry>, RarError> {
        let (v, mut at) = detect(b).ok_or(RarError::NotRar)?;
        let mut out = Vec::new();
        while (at as usize) < b.len() {
            let p = parse_block(v, &b[at as usize..], at)?.map_err(|_| RarError::Corrupt)?;
            match p.block {
                Block::File(e) => out.push(e),
                Block::End => break,
                Block::Other => {}
            }
            at = p.next;
        }
        Ok(out)
    }

    fn data<'a>(b: &'a [u8], e: &Entry) -> &'a [u8] {
        let o = e.data_offset.unwrap() as usize;
        &b[o..o + e.compressed_size as usize]
    }

    // ---------- testes ----------

    #[test]
    fn detects_versions() {
        assert_eq!(detect(&build_rar4(&[], None)), Some((Version::V4, 7)));
        assert_eq!(detect(&build_rar5(&[])), Some((Version::V5, 8)));
        assert_eq!(detect(b"PK\x03\x04"), None);
    }

    #[test]
    fn rar4_lists_stored_files_with_data_offsets() {
        let b = build_rar4(&[("Vol 01\\C_01.rar", b"capitulo-1"), ("Vol 01\\C_02.rar", b"cap-2")], None);
        let files = walk(&b).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "Vol 01/C_01.rar");
        assert_eq!(files[0].method, 0);
        assert_eq!(data(&b, &files[0]), b"capitulo-1");
        assert_eq!(data(&b, &files[1]), b"cap-2");
    }

    #[test]
    fn rar4_unicode_name() {
        // "ação.jpg": ascii "acao.jpg" + codificação unicode do RAR (high byte 0x00).
        // flags 0x40 0x55 0x00 = 4 caracteres de 1 byte com high, depois cópia do ascii.
        let enc = [0x00, 0b0000_0000, b'a', 0xE7, 0xE3, b'o', 0b1100_0000, 0x02];
        let b = build_rar4(&[], Some(("acao.jpg", &enc)));
        let files = walk(&b).unwrap();
        assert_eq!(files[0].name, "ação.jpg");
    }

    #[test]
    fn rar4_asks_for_more_bytes() {
        let b = build_rar4(&[("a.jpg", b"x")], None);
        let (v, at) = detect(&b).unwrap();
        let main = parse_block(v, &b[at as usize..], at).unwrap().unwrap();
        let short = &b[main.next as usize..main.next as usize + 10];
        assert!(matches!(parse_block(v, short, main.next), Ok(Err(_))));
    }

    #[test]
    fn rar4_encrypted_file_is_rejected() {
        let mut b = build_rar4(&[("a.jpg", b"x")], None);
        // flags do cabeçalho do arquivo: bit 0x04 = com senha.
        let file_flags_at = 7 + 13 + 3;
        b[file_flags_at] |= 0x04;
        assert_eq!(walk(&b), Err(RarError::Encrypted));
    }

    #[test]
    fn rar5_lists_stored_and_marks_packed() {
        let b = build_rar5(&[("Cap 1/001.jpg", b"pagina", 0), ("Cap 1/002.jpg", b"zzzz", 3)]);
        let files = walk(&b).unwrap();
        assert_eq!(files[0].name, "Cap 1/001.jpg");
        assert_eq!(files[0].method, 0);
        assert_eq!(data(&b, &files[0]), b"pagina");
        assert_eq!(files[1].method, METHOD_PACKED);
    }

    #[test]
    fn rar_inside_rar() {
        let inner = build_rar4(&[("001.jpg", b"pg-1"), ("002.jpg", b"pg-2")], None);
        let outer = build_rar4(&[("V01\\C_01.rar", &inner)], None);
        let e = &walk(&outer).unwrap()[0];
        let window = data(&outer, e);
        let pages = walk(window).unwrap();
        assert_eq!(data(window, &pages[1]), b"pg-2");
    }
}
