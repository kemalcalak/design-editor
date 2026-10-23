//! On-demand Google Fonts download — the network half.
//!
//! A family is fetched through the CSS v1 API
//! (`fonts.googleapis.com/css?family=<name>:100,…,900i&subset=all`). With a
//! non-browser user agent that endpoint answers with one static `.ttf` URL
//! per face the family actually has (missing weights are skipped, unlike
//! CSS2 which rejects the whole request), and HTTP 400 for an unknown
//! family. `subset=all` returns full files; the default is Latin only, which
//! would drop Turkish, Cyrillic and Vietnamese glyphs.
//!
//! Persisting and registering the faces is the desktop host's job (its
//! `FontStore` owns the disk); this module only turns a family name into
//! font bytes. The transport is a trait so tests run without a network.

use std::time::Duration;

pub use crate::google_fonts_error::GoogleFontsError;

/// CSS v1 endpoint. Google Fonts is one of the network calls the
/// local-first rule allows.
pub const CSS_API: &str = "https://fonts.googleapis.com/css";

/// The only host a listed face may be fetched from.
const FACE_URL_PREFIX: &str = "https://fonts.gstatic.com/";

/// Every weight, upright and italic. The v1 API returns only those the
/// family has.
const ALL_FACES: &str =
    "100,200,300,400,500,600,700,800,900,100i,200i,300i,400i,500i,600i,700i,800i,900i";

/// Cap for the CSS answer (a full family is ~5 KB).
const MAX_CSS_BYTES: usize = 256 * 1024;

/// Cap for one face; matches the desktop `FontStore` import cap. Full CJK
/// faces are a few MiB.
pub const MAX_FACE_BYTES: usize = 16 * 1024 * 1024;

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// A status code plus body, enough for the API's success / 400 split.
pub struct HttpReply {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Blocking GET used by [`download_family`].
pub trait FontTransport: Send + Sync {
    fn get(&self, url: &str, max_bytes: usize) -> Result<HttpReply, GoogleFontsError>;
}

/// The CSS v1 URL that lists every face of `family`.
pub fn css_url(family: &str) -> Result<String, GoogleFontsError> {
    let family = family.trim();
    let valid = !family.is_empty()
        && family
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-');
    if !valid {
        return Err(GoogleFontsError::InvalidFamily {
            family: family.to_string(),
        });
    }
    let name = family.split_whitespace().collect::<Vec<_>>().join("+");
    Ok(format!("{CSS_API}?family={name}:{ALL_FACES}&subset=all"))
}

/// One `@font-face` block of a CSS answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceSource {
    pub url: String,
    /// The block's `font-weight` (400 when absent).
    pub weight: u16,
    /// `font-style: italic`.
    pub italic: bool,
}

/// The value of `property` inside one `@font-face` block.
fn block_value<'a>(block: &'a str, property: &str) -> Option<&'a str> {
    let rest = &block[block.find(property)? + property.len()..];
    let rest = rest.trim_start().strip_prefix(':')?;
    Some(rest[..rest.find(';').unwrap_or(rest.len())].trim())
}

/// The faces a CSS answer lists, in order, deduplicated by URL. Anything not
/// served from `fonts.gstatic.com` is ignored.
pub fn parse_faces(css: &str) -> Vec<FaceSource> {
    let mut faces: Vec<FaceSource> = Vec::new();
    for block in css.split("@font-face").skip(1) {
        let Some(start) = block.find("url(") else {
            continue;
        };
        let rest = &block[start + 4..];
        let Some(end) = rest.find(')') else { continue };
        let url = rest[..end].trim().trim_matches(|c| c == '\'' || c == '"');
        if !url.starts_with(FACE_URL_PREFIX) || faces.iter().any(|face| face.url == url) {
            continue;
        }
        faces.push(FaceSource {
            url: url.to_string(),
            weight: block_value(block, "font-weight")
                .and_then(|value| value.parse().ok())
                .unwrap_or(400),
            italic: block_value(block, "font-style") == Some("italic"),
        });
    }
    faces
}

/// The family name the CSS answer declares (`font-family: 'Space Mono';`),
/// i.e. Google's own spelling of the requested family.
pub fn parse_family_name(css: &str) -> Option<String> {
    let rest = &css[css.find("font-family:")? + "font-family:".len()..];
    let name = rest[..rest.find(';')?]
        .trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Every face of one family, as served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedFamily {
    /// Google's spelling of the family (falls back to the request).
    pub family: String,
    /// One per weight / slant.
    pub faces: Vec<DownloadedFace>,
}

/// One face's bytes plus the weight / slant the CSS declared for it. The
/// declared values are the truth: the served files' own `OS/2` weight is
/// clamped (Thin and ExtraLight both say 250).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedFace {
    pub weight: u16,
    pub italic: bool,
    pub bytes: Vec<u8>,
}

/// `family` with each word's first letter upper-cased ("space mono" →
/// "Space Mono"). The API matches names case-sensitively.
fn title_case(family: &str) -> String {
    family
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Fetch the CSS answer for `family`, retrying title-cased and then with
/// an upper-cased acronym when the exact spelling is unknown.
fn fetch_css(transport: &dyn FontTransport, family: &str) -> Result<String, GoogleFontsError> {
    let family = family.trim();
    let titled = title_case(family);
    // Acronym families ("IBM Plex Sans", "PT Serif", "DM Sans") upper-case
    // their short leading words.
    let acronym = titled
        .split(' ')
        .enumerate()
        .map(|(index, word)| {
            if index == 0 && word.chars().count() <= 3 {
                word.to_uppercase()
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut candidates = vec![family.to_string()];
    for candidate in [titled, acronym] {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    for candidate in candidates {
        let url = css_url(&candidate)?;
        let reply = transport.get(&url, MAX_CSS_BYTES)?;
        match reply.status {
            200 => return Ok(String::from_utf8_lossy(&reply.body).into_owned()),
            400 | 404 => continue,
            status => return Err(GoogleFontsError::HttpStatus { url, status }),
        }
    }
    Err(GoogleFontsError::NotOnGoogle {
        family: family.to_string(),
    })
}

/// Download every face of `family`. All or nothing: one failed face fails
/// the family. The caller validates, persists and registers the bytes.
pub fn download_family(
    transport: &dyn FontTransport,
    family: &str,
) -> Result<DownloadedFamily, GoogleFontsError> {
    let css = fetch_css(transport, family)?;
    let sources = parse_faces(&css);
    if sources.is_empty() {
        return Err(GoogleFontsError::NoFaces {
            family: family.trim().to_string(),
        });
    }
    let faces = sources
        .into_iter()
        .map(|source| {
            let reply = transport.get(&source.url, MAX_FACE_BYTES)?;
            if reply.status != 200 {
                return Err(GoogleFontsError::HttpStatus {
                    url: source.url,
                    status: reply.status,
                });
            }
            Ok(DownloadedFace {
                weight: source.weight,
                italic: source.italic,
                bytes: reply.body,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DownloadedFamily {
        family: parse_family_name(&css).unwrap_or_else(|| family.trim().to_string()),
        faces,
    })
}

/// The real transport: `reqwest` bridged through `block_on_anywhere`, so it
/// is safe on a plain worker thread and on a tokio worker alike.
pub struct HttpFontTransport;

impl FontTransport for HttpFontTransport {
    fn get(&self, url: &str, max_bytes: usize) -> Result<HttpReply, GoogleFontsError> {
        let request_error = |detail: String| GoogleFontsError::Request {
            url: url.to_string(),
            detail,
        };
        crate::chat_runtime::block_on_anywhere(async {
            // A browser user agent would get WOFF2, which skia cannot load;
            // ours gets TrueType.
            let client = reqwest::Client::builder()
                .use_rustls_tls()
                .timeout(REQUEST_TIMEOUT)
                .user_agent(concat!("design-editor/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|e| GoogleFontsError::HttpClient(e.to_string()))?;
            let mut resp = client
                .get(url)
                .send()
                .await
                .map_err(|e| request_error(e.to_string()))?;
            let status = resp.status().as_u16();
            if resp
                .content_length()
                .is_some_and(|len| len > max_bytes as u64)
            {
                return Err(GoogleFontsError::TooLarge {
                    url: url.to_string(),
                });
            }
            let mut body = Vec::new();
            while let Some(chunk) = resp
                .chunk()
                .await
                .map_err(|e| request_error(e.to_string()))?
            {
                if body.len() + chunk.len() > max_bytes {
                    return Err(GoogleFontsError::TooLarge {
                        url: url.to_string(),
                    });
                }
                body.extend_from_slice(&chunk);
            }
            Ok(HttpReply { status, body })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// Canned replies keyed by URL; records every request.
    struct FakeTransport {
        replies: BTreeMap<String, Result<(u16, Vec<u8>), GoogleFontsError>>,
        requested: Mutex<Vec<String>>,
    }

    impl FontTransport for FakeTransport {
        fn get(&self, url: &str, _max: usize) -> Result<HttpReply, GoogleFontsError> {
            self.requested.lock().unwrap().push(url.to_string());
            match self.replies.get(url) {
                Some(Ok((status, body))) => Ok(HttpReply {
                    status: *status,
                    body: body.clone(),
                }),
                Some(Err(error)) => Err(error.clone()),
                None => Ok(HttpReply {
                    status: 404,
                    body: Vec::new(),
                }),
            }
        }
    }

    const CSS: &str = "@font-face {\n  font-family: 'Space Mono';\n  font-style: italic;\n  font-weight: 400;\n  src: url(https://fonts.gstatic.com/s/spacemono/v17/a.ttf) format('truetype');\n}\n@font-face {\n  font-family: 'Space Mono';\n  font-style: normal;\n  font-weight: 700;\n  src: url(https://fonts.gstatic.com/s/spacemono/v17/b.ttf) format('truetype');\n}\n@font-face {\n  src: url(https://evil.example/c.ttf);\n}\n@font-face {\n  src: url('https://fonts.gstatic.com/s/spacemono/v17/b.ttf');\n}\n";

    #[test]
    fn css_url_asks_for_every_face_and_all_subsets() {
        assert_eq!(
            css_url("  Big Shoulders Display ").unwrap(),
            format!("{CSS_API}?family=Big+Shoulders+Display:{ALL_FACES}&subset=all")
        );
    }

    #[test]
    fn css_url_refuses_names_that_could_inject_query_parameters() {
        for bad in ["", "Inter&subset=latin", "Inter:700", "Ïnter", "a/b"] {
            assert!(
                matches!(css_url(bad), Err(GoogleFontsError::InvalidFamily { .. })),
                "{bad:?} must be refused"
            );
        }
    }

    #[test]
    fn parse_keeps_gstatic_faces_in_order_without_duplicates() {
        assert_eq!(
            parse_faces(CSS),
            vec![
                FaceSource {
                    url: "https://fonts.gstatic.com/s/spacemono/v17/a.ttf".into(),
                    weight: 400,
                    italic: true,
                },
                FaceSource {
                    url: "https://fonts.gstatic.com/s/spacemono/v17/b.ttf".into(),
                    weight: 700,
                    italic: false,
                },
            ]
        );
    }

    #[test]
    fn download_fetches_every_listed_face() {
        // The lower-case spelling is unknown (fake 404); the title-cased
        // retry finds the family.
        let css_url = css_url("Space Mono").unwrap();
        let transport = FakeTransport {
            replies: BTreeMap::from([
                (css_url.clone(), Ok((200, CSS.as_bytes().to_vec()))),
                (
                    "https://fonts.gstatic.com/s/spacemono/v17/a.ttf".into(),
                    Ok((200, b"italic".to_vec())),
                ),
                (
                    "https://fonts.gstatic.com/s/spacemono/v17/b.ttf".into(),
                    Ok((200, b"regular".to_vec())),
                ),
            ]),
            requested: Mutex::new(Vec::new()),
        };
        let downloaded = download_family(&transport, "space mono").unwrap();
        assert_eq!(downloaded.family, "Space Mono", "Google's spelling wins");
        let faces: Vec<(bool, &[u8])> = downloaded
            .faces
            .iter()
            .map(|face| (face.italic, face.bytes.as_slice()))
            .collect();
        assert_eq!(
            faces,
            vec![(true, &b"italic"[..]), (false, &b"regular"[..])]
        );
        assert_eq!(transport.requested.lock().unwrap().len(), 4);
    }

    #[test]
    fn unknown_family_is_reported_as_not_on_google() {
        let transport = FakeTransport {
            replies: BTreeMap::from([(css_url("Nope Sans").unwrap(), Ok((400, Vec::new())))]),
            requested: Mutex::new(Vec::new()),
        };
        let error = download_family(&transport, "Nope Sans").unwrap_err();
        assert!(error.is_not_on_google(), "{error}");
    }

    #[test]
    fn offline_is_a_retryable_failure() {
        let url = css_url("Montserrat").unwrap();
        let transport = FakeTransport {
            replies: BTreeMap::from([(
                url.clone(),
                Err(GoogleFontsError::Request {
                    url,
                    detail: "dns error".into(),
                }),
            )]),
            requested: Mutex::new(Vec::new()),
        };
        let error = download_family(&transport, "Montserrat").unwrap_err();
        assert!(!error.is_not_on_google(), "{error}");
    }

    #[test]
    fn a_failed_face_fails_the_whole_family() {
        let css_url = css_url("Space Mono").unwrap();
        let transport = FakeTransport {
            replies: BTreeMap::from([
                (css_url, Ok((200, CSS.as_bytes().to_vec()))),
                (
                    "https://fonts.gstatic.com/s/spacemono/v17/a.ttf".into(),
                    Ok((200, b"italic".to_vec())),
                ),
            ]),
            requested: Mutex::new(Vec::new()),
        };
        assert!(matches!(
            download_family(&transport, "Space Mono"),
            Err(GoogleFontsError::HttpStatus { status: 404, .. })
        ));
    }

    #[test]
    fn acronym_families_are_found_from_a_lower_case_spelling() {
        let transport = FakeTransport {
            replies: BTreeMap::from([(
                css_url("IBM Plex Sans").unwrap(),
                Ok((200, CSS.as_bytes().to_vec())),
            )]),
            requested: Mutex::new(Vec::new()),
        };
        // The faces of the fixture CSS are not served, so the download
        // fails after the CSS lookup: what matters is which spellings ran.
        let _ = download_family(&transport, "ibm plex sans");
        let requested = transport.requested.lock().unwrap();
        assert_eq!(requested[0], css_url("ibm plex sans").unwrap());
        assert_eq!(requested[1], css_url("Ibm Plex Sans").unwrap());
        assert_eq!(requested[2], css_url("IBM Plex Sans").unwrap());
    }
}
