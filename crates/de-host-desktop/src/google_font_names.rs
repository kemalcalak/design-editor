//! Rewrite a downloaded face's family name, weight and slant to what Google
//! Fonts declared for it.
//!
//! The static `.ttf` files the Google Fonts CSS API serves are instances cut
//! from a variable font, and for several families the cut keeps the default
//! instance's name: every Montserrat weight, Regular included, says
//! "Montserrat Thin" in its `name` table. The registry keys a face on that
//! internal name, so registered as-is the face would never answer a document
//! asking for "Montserrat". The family, subfamily, full and PostScript names
//! are therefore replaced; every other record (copyright, licence, designer,
//! URLs) is kept.
//!
//! The same cut clamps `OS/2.usWeightClass` (Thin and ExtraLight both say
//! 250, so one would replace the other in the registry), so the weight and
//! slant the CSS declared are written into `OS/2` and `head.macStyle` too.

use crate::fonts_error::FontNameError;

const NAME: [u8; 4] = *b"name";
const HEAD: [u8; 4] = *b"head";
const OS2: [u8; 4] = *b"OS/2";

/// Name IDs this module owns: family, subfamily, full name, PostScript name,
/// typographic family, typographic subfamily.
const REPLACED_IDS: [u16; 6] = [1, 2, 4, 6, 16, 17];

/// Windows platform, Unicode BMP encoding, US English — the record set every
/// font manager (CoreText, DirectWrite, FreeType) reads.
const WIN_PLATFORM: u16 = 3;
const WIN_UNICODE_BMP: u16 = 1;
const WIN_EN_US: u16 = 0x0409;

struct TableRecord {
    tag: [u8; 4],
    offset: usize,
    length: usize,
}

struct NameRecord {
    platform: u16,
    encoding: u16,
    language: u16,
    name_id: u16,
    bytes: Vec<u8>,
}

/// The identity a downloaded face should register under.
#[derive(Debug, Clone, Copy)]
pub struct FaceIdentity<'a> {
    pub family: &'a str,
    pub weight: u16,
    pub italic: bool,
}

/// `bytes` renamed to `face.family` with `face.weight` / `face.italic`
/// written into `OS/2` and `head`. Fails on anything that is not a
/// single-face TrueType / OpenType file with `name` and `OS/2` tables.
pub fn with_identity(bytes: &[u8], face: FaceIdentity<'_>) -> Result<Vec<u8>, FontNameError> {
    let tables = read_table_directory(bytes)?;
    let find = |tag: [u8; 4]| tables.iter().find(|table| table.tag == tag);
    let name = find(NAME).ok_or(FontNameError::MissingTable("name"))?;
    let os2 = find(OS2).ok_or(FontNameError::MissingTable("OS/2"))?;
    if os2.length < 64 {
        return Err(FontNameError::Truncated);
    }
    let FaceIdentity {
        family,
        weight,
        italic,
    } = face;

    let mut records: Vec<NameRecord> = read_name_records(table_bytes(bytes, name)?)?
        .into_iter()
        .filter(|record| !REPLACED_IDS.contains(&record.name_id))
        .collect();
    let style = style_name(weight, italic);
    let ribbi = match (weight >= 700, italic) {
        (false, false) => "Regular",
        (false, true) => "Italic",
        (true, false) => "Bold",
        (true, true) => "Bold Italic",
    };
    let postscript: String = format!("{family}-{style}")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    for (name_id, text) in [
        (1, family.to_string()),
        (2, ribbi.to_string()),
        (4, format!("{family} {style}")),
        (6, postscript),
        (16, family.to_string()),
        (17, style.to_string()),
    ] {
        records.push(NameRecord {
            platform: WIN_PLATFORM,
            encoding: WIN_UNICODE_BMP,
            language: WIN_EN_US,
            name_id,
            bytes: text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        });
    }
    records.sort_by_key(|r| (r.platform, r.encoding, r.language, r.name_id));

    let new_name = write_name_table(&records)?;
    Ok(rebuild_font(bytes, &tables, &new_name, weight, italic))
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, FontNameError> {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or(FontNameError::Truncated)
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, FontNameError> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(FontNameError::Truncated)
}

fn read_table_directory(bytes: &[u8]) -> Result<Vec<TableRecord>, FontNameError> {
    let version = read_u32(bytes, 0)?;
    // TrueType outlines (0x00010000 / 'true') or CFF ('OTTO'); a collection
    // ('ttcf') or WOFF is refused.
    if !matches!(version, 0x0001_0000 | 0x7472_7565 | 0x4F54_544F) {
        return Err(FontNameError::NotASingleFace);
    }
    let count = read_u16(bytes, 4)? as usize;
    (0..count)
        .map(|index| {
            let at = 12 + index * 16;
            let tag = bytes
                .get(at..at + 4)
                .ok_or(FontNameError::Truncated)?
                .try_into()
                .map_err(|_| FontNameError::Truncated)?;
            let offset = read_u32(bytes, at + 8)? as usize;
            let length = read_u32(bytes, at + 12)? as usize;
            if offset
                .checked_add(length)
                .is_none_or(|end| end > bytes.len())
            {
                return Err(FontNameError::Truncated);
            }
            Ok(TableRecord {
                tag,
                offset,
                length,
            })
        })
        .collect()
}

fn table_bytes<'a>(bytes: &'a [u8], table: &TableRecord) -> Result<&'a [u8], FontNameError> {
    bytes
        .get(table.offset..table.offset + table.length)
        .ok_or(FontNameError::Truncated)
}

fn read_name_records(name: &[u8]) -> Result<Vec<NameRecord>, FontNameError> {
    let count = read_u16(name, 2)? as usize;
    let storage = read_u16(name, 4)? as usize;
    (0..count)
        .map(|index| {
            let at = 6 + index * 12;
            let length = read_u16(name, at + 8)? as usize;
            let offset = storage + read_u16(name, at + 10)? as usize;
            Ok(NameRecord {
                platform: read_u16(name, at)?,
                encoding: read_u16(name, at + 2)?,
                language: read_u16(name, at + 4)?,
                name_id: read_u16(name, at + 6)?,
                bytes: name
                    .get(offset..offset + length)
                    .ok_or(FontNameError::Truncated)?
                    .to_vec(),
            })
        })
        .collect()
}

fn style_name(weight: u16, italic: bool) -> &'static str {
    let rounded = ((weight.clamp(100, 900) + 50) / 100) * 100;
    match (rounded, italic) {
        (100, false) => "Thin",
        (100, true) => "Thin Italic",
        (200, false) => "ExtraLight",
        (200, true) => "ExtraLight Italic",
        (300, false) => "Light",
        (300, true) => "Light Italic",
        (500, false) => "Medium",
        (500, true) => "Medium Italic",
        (600, false) => "SemiBold",
        (600, true) => "SemiBold Italic",
        (700, false) => "Bold",
        (700, true) => "Bold Italic",
        (800, false) => "ExtraBold",
        (800, true) => "ExtraBold Italic",
        (900, false) => "Black",
        (900, true) => "Black Italic",
        (_, false) => "Regular",
        (_, true) => "Italic",
    }
}

fn write_name_table(records: &[NameRecord]) -> Result<Vec<u8>, FontNameError> {
    let header_len = 6 + records.len() * 12;
    let mut out = Vec::with_capacity(header_len);
    let mut storage: Vec<u8> = Vec::new();
    let as_u16 = |value: usize| u16::try_from(value).map_err(|_| FontNameError::NameTooLarge);
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&as_u16(records.len())?.to_be_bytes());
    out.extend_from_slice(&as_u16(header_len)?.to_be_bytes());
    for record in records {
        for value in [
            record.platform,
            record.encoding,
            record.language,
            record.name_id,
            as_u16(record.bytes.len())?,
            as_u16(storage.len())?,
        ] {
            out.extend_from_slice(&value.to_be_bytes());
        }
        storage.extend_from_slice(&record.bytes);
    }
    out.extend_from_slice(&storage);
    Ok(out)
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

/// `OS/2.fsSelection` with the italic / bold / regular bits set for the face.
fn fs_selection(current: u16, weight: u16, italic: bool) -> u16 {
    const ITALIC: u16 = 1 << 0;
    const BOLD: u16 = 1 << 5;
    const REGULAR: u16 = 1 << 6;
    let bold = weight == 700;
    let mut bits = current & !(ITALIC | BOLD | REGULAR);
    if italic {
        bits |= ITALIC;
    }
    if bold {
        bits |= BOLD;
    }
    if !italic && !bold {
        bits |= REGULAR;
    }
    bits
}

/// Re-emit the font with `name` replaced and the style fields of `OS/2` /
/// `head` set: same table order, each table 4-byte aligned, directory
/// checksums recomputed and `head.checkSumAdjustment` fixed up.
fn rebuild_font(
    bytes: &[u8],
    tables: &[TableRecord],
    new_name: &[u8],
    weight: u16,
    italic: bool,
) -> Vec<u8> {
    let header_len = 12 + tables.len() * 16;
    let mut out = bytes[..header_len].to_vec();
    let mut head_at = None;
    for (index, table) in tables.iter().enumerate() {
        let mut data = if table.tag == NAME {
            new_name.to_vec()
        } else {
            bytes[table.offset..table.offset + table.length].to_vec()
        };
        if table.tag == OS2 {
            data[4..6].copy_from_slice(&weight.to_be_bytes());
            let selection = u16::from_be_bytes([data[62], data[63]]);
            data[62..64].copy_from_slice(&fs_selection(selection, weight, italic).to_be_bytes());
        }
        if table.tag == HEAD && data.len() >= 46 {
            data[8..12].fill(0);
            let mac_style = u16::from_be_bytes([data[44], data[45]]) & !0b11
                | u16::from(weight == 700)
                | u16::from(italic) << 1;
            data[44..46].copy_from_slice(&mac_style.to_be_bytes());
            head_at = Some(out.len());
        }
        let offset = out.len() as u32;
        let length = data.len() as u32;
        let sum = checksum(&data);
        let record = 12 + index * 16;
        out[record + 4..record + 8].copy_from_slice(&sum.to_be_bytes());
        out[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
        out[record + 12..record + 16].copy_from_slice(&length.to_be_bytes());
        out.extend_from_slice(&data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    if let Some(head_at) = head_at {
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[head_at + 8..head_at + 12].copy_from_slice(&adjustment.to_be_bytes());
    }
    out
}

// Gated off Windows like `fonts.rs`: skia's DirectWrite font manager is not
// safe to drive from cargo's parallel test threads there.
#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;

    const DM_MONO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/DMMono-Medium.ttf");

    fn identity(weight: u16, italic: bool) -> FaceIdentity<'static> {
        FaceIdentity {
            family: "Renamed Mono",
            weight,
            italic,
        }
    }

    #[test]
    fn renamed_face_reports_the_new_family_and_keeps_its_weight() {
        let before = jian_skia::parse_imported_font_meta(DM_MONO_MEDIUM).expect("fixture parses");
        let renamed =
            with_identity(DM_MONO_MEDIUM, identity(before.weight, false)).expect("rename");
        let after = jian_skia::parse_imported_font_meta(&renamed).expect("renamed face parses");
        assert_eq!(after.family, "Renamed Mono");
        assert_eq!(after.weight, before.weight);
        assert_eq!(after.style, before.style);
    }

    #[test]
    fn declared_weight_and_slant_replace_the_served_ones() {
        let renamed = with_identity(DM_MONO_MEDIUM, identity(200, true)).expect("restyle");
        let after = jian_skia::parse_imported_font_meta(&renamed).expect("restyled face parses");
        assert_eq!(after.weight, 200);
        assert_eq!(
            after.style,
            jian_core::layout::measure::FontStyleKind::Italic
        );
    }

    #[test]
    fn untouched_records_survive_and_checksum_adjustment_is_valid() {
        let renamed = with_identity(DM_MONO_MEDIUM, identity(500, false)).expect("rename");
        let tables = read_table_directory(&renamed).expect("directory");
        let name = tables.iter().find(|t| t.tag == NAME).unwrap();
        let records = read_name_records(table_bytes(&renamed, name).unwrap()).unwrap();
        let original = read_table_directory(DM_MONO_MEDIUM).unwrap();
        let original_name = original.iter().find(|t| t.tag == NAME).unwrap();
        let original_records =
            read_name_records(table_bytes(DM_MONO_MEDIUM, original_name).unwrap()).unwrap();
        for kept in original_records
            .iter()
            .filter(|r| !REPLACED_IDS.contains(&r.name_id))
        {
            assert!(
                records.iter().any(|r| r.name_id == kept.name_id
                    && r.platform == kept.platform
                    && r.bytes == kept.bytes),
                "name record {} must be kept",
                kept.name_id
            );
        }
        assert_eq!(checksum(&renamed), 0xB1B0_AFBA, "whole-font checksum");
    }

    #[test]
    fn non_font_bytes_are_refused() {
        assert!(with_identity(b"not a font at all", identity(400, false)).is_err());
        assert!(with_identity(b"ttcf\0\0\0\0\0\0\0\0", identity(400, false)).is_err());
    }

    #[test]
    fn style_names_follow_weight_and_slant() {
        assert_eq!(style_name(400, false), "Regular");
        assert_eq!(style_name(400, true), "Italic");
        assert_eq!(style_name(500, false), "Medium");
        assert_eq!(style_name(900, true), "Black Italic");
        assert_eq!(fs_selection(0, 400, false), 1 << 6);
        assert_eq!(fs_selection(1 << 6, 700, true), 1 | 1 << 5);
    }
}
