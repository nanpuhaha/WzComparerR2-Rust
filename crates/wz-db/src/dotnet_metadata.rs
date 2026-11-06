//! 최소 .NET PE/CLI 메타데이터 리더.
//!
//! `PluginManager.LoadPlugin`이 필요로 하는 범위만 읽는다.
//! - 어셈블리 이름/버전, `AssemblyTitle/Company/InformationalVersion/FileVersion`
//! - public 타입과 그 기반 타입 이름 (`PluginEntry` 서브클래스 판별용)

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DotnetAssembly {
    pub name: String,
    pub version: Option<String>,
    pub title: Option<String>,
    pub company: Option<String>,
    pub informational_version: Option<String>,
    pub file_version: Option<String>,
    /// public 타입의 `(namespace.name, base type full name)`.
    pub public_types: Vec<PublicType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicType {
    pub name: String,
    pub base_type: Option<String>,
    pub is_abstract: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Col {
    U16,
    U32,
    Str,
    Guid,
    Blob,
    Table(u8),
    Coded(u8),
}

const TYPE_DEF_OR_REF: u8 = 0;
const HAS_CONSTANT: u8 = 1;
const HAS_CUSTOM_ATTRIBUTE: u8 = 2;
const HAS_FIELD_MARSHAL: u8 = 3;
const HAS_DECL_SECURITY: u8 = 4;
const MEMBER_REF_PARENT: u8 = 5;
const HAS_SEMANTICS: u8 = 6;
const METHOD_DEF_OR_REF: u8 = 7;
const MEMBER_FORWARDED: u8 = 8;
const IMPLEMENTATION: u8 = 9;
const CUSTOM_ATTRIBUTE_TYPE: u8 = 10;
const RESOLUTION_SCOPE: u8 = 11;
const TYPE_OR_METHOD_DEF: u8 = 12;

fn coded_tables(kind: u8) -> &'static [u8] {
    match kind {
        TYPE_DEF_OR_REF => &[2, 1, 27],
        HAS_CONSTANT => &[4, 8, 23],
        HAS_CUSTOM_ATTRIBUTE => &[
            6, 4, 1, 2, 8, 9, 10, 0, 14, 23, 20, 17, 26, 27, 32, 35, 38, 39, 40, 42, 44, 43,
        ],
        HAS_FIELD_MARSHAL => &[4, 8],
        HAS_DECL_SECURITY => &[2, 6, 32],
        MEMBER_REF_PARENT => &[2, 1, 26, 6, 27],
        HAS_SEMANTICS => &[20, 23],
        METHOD_DEF_OR_REF => &[6, 10],
        MEMBER_FORWARDED => &[4, 6],
        IMPLEMENTATION => &[38, 35, 39],
        CUSTOM_ATTRIBUTE_TYPE => &[6, 10],
        RESOLUTION_SCOPE => &[0, 26, 35, 1],
        TYPE_OR_METHOD_DEF => &[2, 6],
        _ => &[],
    }
}

fn coded_bits(kind: u8) -> u32 {
    match kind {
        TYPE_DEF_OR_REF | HAS_CONSTANT | HAS_DECL_SECURITY | IMPLEMENTATION | RESOLUTION_SCOPE => 2,
        HAS_CUSTOM_ATTRIBUTE => 5,
        HAS_FIELD_MARSHAL | HAS_SEMANTICS | METHOD_DEF_OR_REF | MEMBER_FORWARDED
        | TYPE_OR_METHOD_DEF => 1,
        MEMBER_REF_PARENT | CUSTOM_ATTRIBUTE_TYPE => 3,
        _ => 0,
    }
}

fn table_schema(table: u8) -> &'static [Col] {
    use Col::*;
    match table {
        0 => &[U16, Str, Guid, Guid, Guid],
        1 => &[Coded(RESOLUTION_SCOPE), Str, Str],
        2 => &[U32, Str, Str, Coded(TYPE_DEF_OR_REF), Table(4), Table(6)],
        3 => &[Table(4)],
        4 => &[U16, Str, Blob],
        5 => &[Table(6)],
        6 => &[U32, U16, U16, Str, Blob, Table(8)],
        7 => &[Table(8)],
        8 => &[U16, U16, Str],
        9 => &[Table(2), Coded(TYPE_DEF_OR_REF)],
        10 => &[Coded(MEMBER_REF_PARENT), Str, Blob],
        11 => &[U16, Coded(HAS_CONSTANT), Blob],
        12 => &[
            Coded(HAS_CUSTOM_ATTRIBUTE),
            Coded(CUSTOM_ATTRIBUTE_TYPE),
            Blob,
        ],
        13 => &[Coded(HAS_FIELD_MARSHAL), Blob],
        14 => &[U16, Coded(HAS_DECL_SECURITY), Blob],
        15 => &[U16, U32, Table(2)],
        16 => &[U32, Table(4)],
        17 => &[Blob],
        18 => &[Table(2), Table(20)],
        19 => &[Table(20)],
        20 => &[U16, Str, Coded(TYPE_DEF_OR_REF)],
        21 => &[Table(2), Table(23)],
        22 => &[Table(23)],
        23 => &[U16, Str, Blob],
        24 => &[U16, Table(6), Coded(HAS_SEMANTICS)],
        25 => &[Table(2), Coded(METHOD_DEF_OR_REF), Coded(METHOD_DEF_OR_REF)],
        26 => &[Str],
        27 => &[Blob],
        28 => &[U16, Coded(MEMBER_FORWARDED), Str, Table(26)],
        29 => &[U32, Table(4)],
        30 => &[U32, U32],
        31 => &[U32],
        32 => &[U32, U16, U16, U16, U16, U32, Blob, Str, Str],
        33 => &[U32],
        34 => &[U32, U32, U32],
        35 => &[U16, U16, U16, U16, U32, Blob, Str, Str, Blob],
        36 => &[U32, Table(35)],
        37 => &[U32, U32, U32, Table(35)],
        38 => &[U32, Str, Blob],
        39 => &[U32, U32, Str, Str, Coded(IMPLEMENTATION)],
        40 => &[U32, U32, Str, Coded(IMPLEMENTATION)],
        41 => &[Table(2), Table(2)],
        42 => &[U16, U16, Coded(TYPE_OR_METHOD_DEF), Str],
        43 => &[Coded(METHOD_DEF_OR_REF), Blob],
        44 => &[Table(42), Coded(TYPE_DEF_OR_REF)],
        _ => &[],
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }
    fn u8(&mut self) -> Option<u8> {
        let v = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        let b = self.data.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        let b = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Option<u64> {
        let b = self.data.get(self.pos..self.pos + 8)?;
        self.pos += 8;
        Some(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.data.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(b)
    }
}

struct Sections {
    entries: Vec<(u32, u32, u32)>,
}

impl Sections {
    fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        for (va, vsize, raw) in &self.entries {
            let size = (*vsize).max(1);
            if rva >= *va && rva < va.saturating_add(size) {
                return Some((rva - va + raw) as usize);
            }
        }
        None
    }
}

struct Metadata<'a> {
    data: &'a [u8],
    strings: (usize, usize),
    blobs: (usize, usize),
    tables: (usize, usize),
}

impl<'a> Metadata<'a> {
    fn string(&self, index: u32) -> Option<String> {
        let base = self.strings.0;
        let end = self.strings.0 + self.strings.1;
        let start = base + index as usize;
        if start >= end {
            return None;
        }
        let bytes = &self.data[start..end];
        let len = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        Some(String::from_utf8_lossy(&bytes[..len]).to_string())
    }

    fn blob(&self, index: u32) -> Option<&'a [u8]> {
        let base = self.blobs.0 + index as usize;
        let end = self.blobs.0 + self.blobs.1;
        if base >= end {
            return None;
        }
        let mut r = Reader::new(&self.data[..end], base);
        let len = read_compressed(&mut r)?;
        r.take(len as usize)
    }
}

fn read_compressed(r: &mut Reader<'_>) -> Option<u32> {
    let b0 = r.u8()?;
    if b0 & 0x80 == 0 {
        return Some(u32::from(b0));
    }
    if b0 & 0xC0 == 0x80 {
        let b1 = r.u8()?;
        return Some((u32::from(b0 & 0x3F) << 8) | u32::from(b1));
    }
    let b1 = r.u8()?;
    let b2 = r.u8()?;
    let b3 = r.u8()?;
    Some(
        (u32::from(b0 & 0x1F) << 24) | (u32::from(b1) << 16) | (u32::from(b2) << 8) | u32::from(b3),
    )
}

struct Tables<'a> {
    data: &'a [u8],
    row_counts: BTreeMap<u8, u32>,
    string_size: u8,
    guid_size: u8,
    blob_size: u8,
}

impl<'a> Tables<'a> {
    fn index_size(&self, table: u8) -> usize {
        if self.row_counts.get(&table).copied().unwrap_or(0) > 0xFFFF {
            4
        } else {
            2
        }
    }

    fn coded_size(&self, kind: u8) -> usize {
        let max = coded_tables(kind)
            .iter()
            .map(|t| self.row_counts.get(t).copied().unwrap_or(0))
            .max()
            .unwrap_or(0);
        let bits = coded_bits(kind);
        if u64::from(max) < (1u64 << (16 - bits)) {
            2
        } else {
            4
        }
    }

    fn col_size(&self, col: Col) -> usize {
        match col {
            Col::U16 => 2,
            Col::U32 => 4,
            Col::Str => self.string_size as usize,
            Col::Guid => self.guid_size as usize,
            Col::Blob => self.blob_size as usize,
            Col::Table(t) => self.index_size(t),
            Col::Coded(k) => self.coded_size(k),
        }
    }

    fn rows_start(&self, table: u8) -> Option<usize> {
        let mut cursor = self.tables_rows_offset()?;
        for t in 0..table {
            let count = self.row_counts.get(&t).copied().unwrap_or(0) as usize;
            let width: usize = table_schema(t).iter().map(|c| self.col_size(*c)).sum();
            cursor += count * width;
        }
        Some(cursor)
    }

    fn tables_rows_offset(&self) -> Option<usize> {
        // `data` starts at the tables stream; recompute header layout.
        let mut r = Reader::new(self.data, 0);
        r.u32()?; // reserved
        r.u8()?; // major
        r.u8()?; // minor
        r.u8()?; // heap sizes
        r.u8()?; // reserved
        let valid = r.u64()?;
        r.u64()?; // sorted
        for t in 0..64u8 {
            if valid & (1u64 << t) != 0 {
                r.u32()?;
            }
        }
        Some(r.pos)
    }

    fn row(&self, table: u8, index: u32) -> Option<Vec<u64>> {
        let count = self.row_counts.get(&table).copied().unwrap_or(0);
        if index == 0 || index > count {
            return None;
        }
        let start = self.rows_start(table)?;
        let width: usize = table_schema(table).iter().map(|c| self.col_size(*c)).sum();
        let mut r = Reader::new(self.data, start + (index as usize - 1) * width);
        let mut out = Vec::new();
        for col in table_schema(table) {
            let v = match col {
                Col::U16 => u64::from(r.u16()?),
                Col::U32 => u64::from(r.u32()?),
                _ => {
                    let size = self.col_size(*col);
                    if size == 2 {
                        u64::from(r.u16()?)
                    } else {
                        u64::from(r.u32()?)
                    }
                }
            };
            out.push(v);
        }
        Some(out)
    }
}

fn decompress(index: u32, kind: u8) -> (u8, u32) {
    let bits = coded_bits(kind);
    let tag = (index & ((1 << bits) - 1)) as u8;
    (tag, index >> bits)
}

/// PE/CLI 메타데이터를 읽어 어셈블리 정보/공개 타입을 돌려준다.
pub fn read_assembly(path: &std::path::Path) -> Result<DotnetAssembly, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let sections = parse_sections(&data)?;
    let cli_rva = cli_directory(&data)?;
    let cli_off = sections
        .rva_to_offset(cli_rva)
        .ok_or("CLI header RVA not mapped")?;
    let mut r = Reader::new(&data, cli_off);
    r.u32().ok_or("cli header")?;
    r.u16().ok_or("cli header")?;
    r.u16().ok_or("cli header")?;
    let meta_rva = r.u32().ok_or("metadata rva")?;
    let meta_size = r.u32().ok_or("metadata size")?;
    let meta_off = sections
        .rva_to_offset(meta_rva)
        .ok_or("metadata RVA not mapped")?;
    let metadata = parse_metadata(&data, meta_off, meta_size as usize)?;
    let tables = parse_tables(&metadata)?;

    let assembly_row = tables.row(32, 1).ok_or("assembly row")?;
    let version = format!(
        "{}.{}.{}.{}",
        assembly_row[1], assembly_row[2], assembly_row[3], assembly_row[4]
    );
    let name = metadata
        .string((assembly_row[7] & 0xFFFF) as u32)
        .or_else(|| metadata.string(assembly_row[7] as u32))
        .unwrap_or_default();

    let mut out = DotnetAssembly {
        name,
        version: Some(version),
        ..Default::default()
    };
    // Assembly custom attributes.
    let ca_count = tables.row_counts.get(&12).copied().unwrap_or(0);
    for i in 1..=ca_count {
        let Some(row) = tables.row(12, i) else {
            continue;
        };
        let (parent_tag, parent_idx) = decompress(row[0] as u32, HAS_CUSTOM_ATTRIBUTE);
        if parent_tag != 14 || parent_idx != 1 {
            continue; // Assembly only
        }
        let Some((ns, ty)) = attribute_type(&tables, &metadata, row[1] as u32) else {
            continue;
        };
        if ns != "System.Reflection" {
            continue;
        }
        let Some(blob) = metadata.blob(row[2] as u32) else {
            continue;
        };
        let Some(value) = first_string_arg(blob) else {
            continue;
        };
        match ty.as_str() {
            "AssemblyTitleAttribute" => out.title = Some(value),
            "AssemblyCompanyAttribute" => out.company = Some(value),
            "AssemblyInformationalVersionAttribute" => out.informational_version = Some(value),
            "AssemblyFileVersionAttribute" => out.file_version = Some(value),
            _ => {}
        }
    }

    // Public types (top-level public, plus public nested of public types).
    let nested = nested_map(&tables);
    let mut public_ids = Vec::new();
    let type_count = tables.row_counts.get(&2).copied().unwrap_or(0);
    for i in 1..=type_count {
        if let Some((_, visible)) = type_visibility(&tables, &nested, i) {
            if visible {
                public_ids.push(i);
            }
        }
    }
    for id in public_ids {
        let Some(row) = tables.row(2, id) else {
            continue;
        };
        let ty_name = metadata.string(row[1] as u32).unwrap_or_default();
        let ns = metadata.string(row[2] as u32).unwrap_or_default();
        let base = base_type_name(&tables, &metadata, row[3] as u32);
        out.public_types.push(PublicType {
            name: if ns.is_empty() {
                ty_name
            } else {
                format!("{ns}.{ty_name}")
            },
            base_type: base,
            is_abstract: (row[0] as u32 & 0x80) != 0,
        });
    }
    out.public_types.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn parse_sections(data: &[u8]) -> Result<Sections, String> {
    let mut r = Reader::new(data, 0x3C);
    let pe_off = r.u32().ok_or("e_lfanew")? as usize;
    if data.get(pe_off..pe_off + 4) != Some(b"PE\0\0") {
        return Err("missing PE signature".into());
    }
    let coff = pe_off + 4;
    let mut r = Reader::new(data, coff);
    r.u16().ok_or("machine")?;
    let nsections = r.u16().ok_or("sections")? as usize;
    r.u32().ok_or("timestamp")?;
    r.u32().ok_or("symtab")?;
    r.u32().ok_or("symbols")?;
    let opt_size = r.u16().ok_or("opt size")? as usize;
    let opt_off = coff + 20;
    let mut entries = Vec::new();
    let sec_off = opt_off + opt_size;
    for i in 0..nsections {
        let off = sec_off + i * 40;
        let mut r = Reader::new(data, off);
        r.take(8).ok_or("section name")?;
        let vsize = r.u32().ok_or("vsize")?;
        let va = r.u32().ok_or("va")?;
        let raw_size = r.u32().ok_or("raw size")?;
        let raw_ptr = r.u32().ok_or("raw ptr")?;
        entries.push((va, vsize.max(raw_size), raw_ptr));
    }
    Ok(Sections { entries })
}

fn cli_directory(data: &[u8]) -> Result<u32, String> {
    let mut r = Reader::new(data, 0x3C);
    let pe_off = r.u32().ok_or("e_lfanew")? as usize;
    let opt_off = pe_off + 24;
    let magic = u16::from_le_bytes([
        *data.get(opt_off).ok_or("magic")?,
        *data.get(opt_off + 1).ok_or("magic")?,
    ]);
    let dd_off = if magic == 0x20B {
        opt_off + 0x70
    } else {
        opt_off + 0x60
    };
    let cli = dd_off + 14 * 8;
    let mut r = Reader::new(data, cli);
    r.u32().ok_or_else(|| "cli rva".to_string())
}

fn parse_metadata<'a>(data: &'a [u8], offset: usize, size: usize) -> Result<Metadata<'a>, String> {
    let end = offset + size;
    if end > data.len() {
        return Err("metadata out of range".into());
    }
    let mut r = Reader::new(data, offset);
    if r.u32() != Some(0x424A_5342) {
        return Err("bad metadata signature".into());
    }
    r.u16().ok_or("major")?;
    r.u16().ok_or("minor")?;
    r.u32().ok_or("reserved")?;
    let version_len = r.u32().ok_or("version len")? as usize;
    r.take(version_len).ok_or("version")?;
    r.u16().ok_or("flags")?;
    let streams = r.u16().ok_or("streams")? as usize;
    let mut strings = (0usize, 0usize);
    let mut blobs = (0usize, 0usize);
    let mut tables = (0usize, 0usize);
    for _ in 0..streams {
        let off = r.u32().ok_or("stream offset")? as usize;
        let len = r.u32().ok_or("stream size")? as usize;
        let start = r.pos;
        while r.pos < end && data.get(r.pos) != Some(&0) {
            r.pos += 1;
        }
        let name = String::from_utf8_lossy(&data[start..r.pos]).to_string();
        r.pos += 1;
        r.pos = (r.pos + 3) & !3;
        let abs = offset + off;
        match name.as_str() {
            "#~" | "#-" => tables = (abs, len),
            "#Strings" => strings = (abs, len),
            "#Blob" => blobs = (abs, len),
            _ => {}
        }
    }
    Ok(Metadata {
        data,
        strings,
        blobs,
        tables,
    })
}

fn parse_tables<'a>(metadata: &'a Metadata<'a>) -> Result<Tables<'a>, String> {
    let (off, len) = metadata.tables;
    let data = metadata.data.get(off..off + len).ok_or("tables stream")?;
    let mut r = Reader::new(data, 0);
    r.u32().ok_or("reserved")?;
    let major = r.u8().ok_or("major")?;
    r.u8().ok_or("minor")?;
    let heap = r.u8().ok_or("heap")?;
    let _ = major;
    r.u8().ok_or("reserved2")?;
    let valid = r.u64().ok_or("valid")?;
    r.u64().ok_or("sorted")?;
    let mut rows = BTreeMap::new();
    for t in 0..64u8 {
        if valid & (1u64 << t) != 0 {
            rows.insert(t, r.u32().ok_or("rows")?);
        }
    }
    Ok(Tables {
        data,
        row_counts: rows,
        string_size: if heap & 0x01 != 0 { 4 } else { 2 },
        guid_size: if heap & 0x02 != 0 { 4 } else { 2 },
        blob_size: if heap & 0x04 != 0 { 4 } else { 2 },
    })
}

fn type_ref_name(tables: &Tables<'_>, metadata: &Metadata<'_>, index: u32) -> Option<String> {
    let row = tables.row(1, index)?;
    let name = metadata.string(row[1] as u32)?;
    let ns = metadata.string(row[2] as u32)?;
    Some(if ns.is_empty() {
        name
    } else {
        format!("{ns}.{name}")
    })
}

fn base_type_name(tables: &Tables<'_>, metadata: &Metadata<'_>, coded: u32) -> Option<String> {
    let (tag, index) = decompress(coded, TYPE_DEF_OR_REF);
    match tag {
        1 => type_ref_name(tables, metadata, index),
        0 => {
            let row = tables.row(2, index)?;
            let name = metadata.string(row[1] as u32)?;
            let ns = metadata.string(row[2] as u32)?;
            Some(if ns.is_empty() {
                name
            } else {
                format!("{ns}.{name}")
            })
        }
        _ => None,
    }
}

fn nested_map(tables: &Tables<'_>) -> BTreeMap<u32, u32> {
    let mut map = BTreeMap::new();
    let count = tables.row_counts.get(&41).copied().unwrap_or(0);
    for i in 1..=count {
        if let Some(row) = tables.row(41, i) {
            map.insert(row[0] as u32, row[1] as u32);
        }
    }
    map
}

/// `(visibility, is_public_exported)`.
fn type_visibility(
    tables: &Tables<'_>,
    nested: &BTreeMap<u32, u32>,
    id: u32,
) -> Option<(u32, bool)> {
    let row = tables.row(2, id)?;
    let flags = row[0] as u32;
    let vis = flags & 0x7;
    if vis == 1 {
        return Some((vis, true));
    }
    if vis == 2 {
        let parent = nested.get(&id)?;
        let (_, parent_public) = type_visibility(tables, nested, *parent)?;
        return Some((vis, parent_public));
    }
    Some((vis, false))
}

fn attribute_type(
    tables: &Tables<'_>,
    metadata: &Metadata<'_>,
    coded: u32,
) -> Option<(String, String)> {
    let (tag, index) = decompress(coded, CUSTOM_ATTRIBUTE_TYPE);
    match tag {
        3 => {
            // MemberRef → Class
            let row = tables.row(10, index)?;
            let (class_tag, class_index) = decompress(row[0] as u32, MEMBER_REF_PARENT);
            let full = match class_tag {
                1 => type_ref_name(tables, metadata, class_index)?,
                0 => {
                    let row = tables.row(2, class_index)?;
                    let name = metadata.string(row[1] as u32)?;
                    let ns = metadata.string(row[2] as u32)?;
                    if ns.is_empty() {
                        name
                    } else {
                        format!("{ns}.{name}")
                    }
                }
                _ => return None,
            };
            let (ns, name) = full.rsplit_once('.').unwrap_or(("", full.as_str()));
            Some((ns.to_string(), name.to_string()))
        }
        _ => None,
    }
}

fn first_string_arg(blob: &[u8]) -> Option<String> {
    let mut r = Reader::new(blob, 0);
    if r.u16()? != 0x0001 {
        return None;
    }
    let len = read_compressed(&mut r)?;
    if len == 0xFF {
        return None;
    }
    let bytes = r.take(len as usize)?;
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// `PluginEntry` 서브클래스(비추상)인 public 타입 이름.
pub fn plugin_types(assembly: &DotnetAssembly) -> Vec<String> {
    let by_name: BTreeMap<&str, &PublicType> = assembly
        .public_types
        .iter()
        .map(|t| (t.name.as_str(), t))
        .collect();
    assembly
        .public_types
        .iter()
        .filter(|t| !t.is_abstract)
        .filter(|t| is_plugin_entry(&t.name, &by_name, 0))
        .map(|t| t.name.clone())
        .collect()
}

fn is_plugin_entry(name: &str, by_name: &BTreeMap<&str, &PublicType>, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    let Some(ty) = by_name.get(name) else {
        return false;
    };
    let Some(base) = ty.base_type.as_deref() else {
        return false;
    };
    if base == "WzComparerR2.PluginBase.PluginEntry" {
        return true;
    }
    by_name
        .get(base)
        .map(|_| is_plugin_entry(base, by_name, depth + 1))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fixture_assembly() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/WzComparerR2.FixturePlugin.dll");
        if !path.exists() {
            return;
        }
        let asm = read_assembly(&path).unwrap();
        assert_eq!(asm.name, "WzComparerR2.FixturePlugin");
        assert_eq!(asm.version.as_deref(), Some("1.2.3.4"));
        assert_eq!(asm.title.as_deref(), Some("픽스처 플러그인"));
        assert_eq!(asm.company.as_deref(), Some("테스트 작성자"));
        assert_eq!(asm.informational_version.as_deref(), Some("9.9.9-beta"));
        assert_eq!(asm.file_version.as_deref(), Some("5.6.7.8"));
        assert_eq!(
            plugin_types(&asm),
            vec!["Fixture.AlphaPlugin", "Fixture.DerivedPlugin"]
        );
    }
}
