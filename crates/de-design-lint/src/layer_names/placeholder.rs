//! Placeholder labels in the 15 editor languages: a label that asks for a
//! piece of content (`Drop logo`, `Your image here`, `Logo placeholder`,
//! `ロゴをここにドロップ`, `Logo hier ablegen`) marks a slot for it.
//!
//! A label is a placeholder when it names a kind of content a slot holds
//! (a logo, an image, a photo, a video, an avatar, a sponsor…) and carries
//! a word that asks for it (`drop`, `here`, `your`, `placeholder`…). Both
//! are needed: `Drop shadow`, `Drop down` and `Upload image` are no slots.

/// Kinds of content a slot holds, in the 15 languages.
const THINGS: &[&str] = &[
    // en
    "logo",
    "image",
    "photo",
    "picture",
    "avatar",
    "video",
    "sponsor",
    "banner",
    "illustration",
    "media", // zh-CN
    "标志",
    "徽标",
    "图片",
    "图像",
    "照片",
    "头像",
    "视频",
    "赞助商",
    "横幅", // zh-TW
    "標誌",
    "圖片",
    "圖像",
    "頭像",
    "影片",
    "贊助商", // ja
    "ロゴ",
    "画像",
    "写真",
    "アバター",
    "動画",
    "スポンサー",
    "バナー", // ko
    "로고",
    "이미지",
    "사진",
    "아바타",
    "동영상",
    "스폰서",
    "배너", // fr / es / pt / de / it-like
    "vidéo",
    "bannière",
    "imagen",
    "foto",
    "vídeo",
    "patrocinador",
    "imagem",
    "bild",
    "fotografía", // ru
    "логотип",
    "изображение",
    "фото",
    "видео",
    "аватар",
    "спонсор",
    "баннер", // hi
    "लोगो",
    "छवि",
    "फ़ोटो",
    "फोटो",
    "वीडियो",
    "प्रायोजक", // tr
    "görsel",
    "resim",
    "fotoğraf", // th
    "โลโก้",
    "รูปภาพ",
    "วิดีโอ",
    "ผู้สนับสนุน", // vi
    "hình ảnh",
    "ảnh",
    "nhà tài trợ", // id
    "gambar",
];

/// Words that ask for the content, in the 15 languages.
const ASKS: &[&str] = &[
    // en
    "drop",
    "here",
    "your",
    "placeholder", // zh-CN
    "拖放",
    "拖入",
    "拖到",
    "放置",
    "此处",
    "这里",
    "占位", // zh-TW
    "拖曳",
    "此處",
    "這裡",
    "預留", // ja
    "ドロップ",
    "ここ",
    "プレースホルダー", // ko
    "드롭",
    "여기",
    "놓기",
    "자리", // fr
    "déposez",
    "déposer",
    "ici",
    "votre",
    "emplacement", // es
    "suelta",
    "arrastra",
    "aquí",
    "tu",
    "su", // de
    "ablegen",
    "hier",
    "ihr",
    "dein",
    "platzhalter", // pt
    "solte",
    "arraste",
    "aqui",
    "seu",
    "sua", // ru
    "перетащите",
    "сюда",
    "ваш",
    "ваше",
    "место", // hi
    "यहाँ",
    "यहां",
    "छोड़ें",
    "अपना", // tr
    "bırak",
    "bırakın",
    "buraya",
    "sürükle",
    "yer tutucu", // th
    "วาง",
    "ที่นี่",
    "ของคุณ", // vi
    "thả",
    "đây",
    "của bạn", // id
    "letakkan",
    "seret",
    "di sini",
    "anda",
];

/// A placeholder label is short: a few words, or a short run in a script
/// written without spaces.
const MAX_WORDS: usize = 5;
const MAX_UNSPACED_CHARS: usize = 16;

/// Scripts written without spaces between words (Han, Kana, Hangul
/// particles, Thai), where a word is found inside the run.
fn unspaced(word: &str) -> bool {
    word.chars().any(|c| {
        matches!(c,
            '\u{3040}'..='\u{30FF}' // kana
            | '\u{3400}'..='\u{9FFF}' // Han
            | '\u{AC00}'..='\u{D7AF}' // Hangul
            | '\u{0E00}'..='\u{0E7F}') // Thai
    })
}

/// The label's words, lowercased, split at anything but letters and digits.
fn words(label: &str) -> Vec<String> {
    label
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && !is_mark(c))
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

/// Combining marks (Devanagari, Thai and Vietnamese vowel signs) belong to
/// their word.
fn is_mark(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{0900}'..='\u{097F}' | '\u{0E30}'..='\u{0E4E}')
}

/// Whether `word` (one or more words) is in `label`: inside the run for an
/// unspaced script, else as whole words.
fn says(lower: &str, spaced: &str, word: &str) -> bool {
    if unspaced(word) {
        lower.contains(word)
    } else {
        spaced.contains(&format!(" {word} "))
    }
}

/// The content a placeholder label asks for, as the label writes it
/// (`Logo` for `Drop logo`), or `None` when the label is no placeholder.
pub(crate) fn slot_thing(label: &str) -> Option<String> {
    let lower = label.to_lowercase();
    let tokens = words(label);
    let long = if unspaced(&lower) {
        lower.chars().filter(|c| !c.is_whitespace()).count() > MAX_UNSPACED_CHARS
    } else {
        tokens.len() > MAX_WORDS
    };
    if tokens.is_empty() || long {
        return None;
    }
    let spaced = format!(" {} ", tokens.join(" "));
    let thing = THINGS.iter().find(|thing| says(&lower, &spaced, thing))?;
    if !ASKS.iter().any(|ask| says(&lower, &spaced, ask)) {
        return None;
    }
    let mut chars = thing.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

#[cfg(test)]
mod tests {
    use super::slot_thing;

    #[test]
    fn placeholders_name_what_they_hold() {
        assert_eq!(slot_thing("Drop logo").as_deref(), Some("Logo"));
        assert_eq!(slot_thing("Your image here").as_deref(), Some("Image"));
        assert_eq!(slot_thing("Logo placeholder").as_deref(), Some("Logo"));
        assert_eq!(slot_thing("Logo hier ablegen").as_deref(), Some("Logo"));
        assert_eq!(slot_thing("ロゴをここにドロップ").as_deref(), Some("ロゴ"));
        assert_eq!(slot_thing("Logonuzu buraya bırakın"), None);
        assert_eq!(slot_thing("Logo buraya bırakın").as_deref(), Some("Logo"));
        assert_eq!(slot_thing("Thả ảnh vào đây").as_deref(), Some("Ảnh"));
    }

    #[test]
    fn actions_and_other_drops_are_no_placeholders() {
        for label in [
            "Drop shadow",
            "Drop down",
            "Upload image",
            "Your email here",
            "Place order",
            "Logo",
        ] {
            assert_eq!(slot_thing(label), None, "{label}");
        }
    }
}
