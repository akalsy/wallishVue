use std::io::Cursor;
use std::time::Duration;

use exif::{Context, In, Reader, Tag};
use futures_util::StreamExt;
use regex::Regex;
use serde::Deserialize;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Clone, Debug, Default)]
pub struct WordMeta {
    pub word: Option<String>,
    pub dictionary_url: Option<String>,
    pub audio_url: Option<String>,
    pub copyright: Option<String>,
    pub copyright_url: Option<String>,
    pub image_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BingImage {
    pub copyright: String,
    pub url: String,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn field_text(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    // ASCII 字段里经常直接塞 UTF-8。display_value 会把非 ASCII 字节印成 \xNN。
    let text = match &field.value {
        exif::Value::Ascii(parts) => join_decoded(parts),
        exif::Value::Byte(bytes) | exif::Value::Undefined(bytes, _) => decode_bytes(bytes),
        _ => field.display_value().to_string(),
    };
    let text = text.trim().trim_matches('"').trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn join_decoded(parts: &[Vec<u8>]) -> String {
    parts
        .iter()
        .map(|part| decode_bytes(part))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn decode_bytes(bytes: &[u8]) -> String {
    let bytes = match bytes.iter().position(|byte| *byte == 0) {
        Some(end) => &bytes[..end],
        None => bytes,
    };
    String::from_utf8(bytes.to_vec())
        .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
        .trim()
        .to_string()
}

pub fn restore_exif_text(text: &str) -> String {
    if !text.contains("\\x") {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            match bytes[index + 1] {
                b'x' if index + 3 < bytes.len() => {
                    if let Ok(hex) = std::str::from_utf8(&bytes[index + 2..index + 4]) {
                        if let Ok(byte) = u8::from_str_radix(hex, 16) {
                            out.push(byte);
                            index += 4;
                            continue;
                        }
                    }
                }
                b'\\' => {
                    out.push(b'\\');
                    index += 2;
                    continue;
                }
                b'"' => {
                    out.push(b'"');
                    index += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

fn split_copyright(raw: Option<String>) -> (Option<String>, Option<String>) {
    let Some(raw) = raw else {
        return (None, None);
    };
    if !raw.contains("||") {
        let text = raw.trim().to_string();
        return (if text.is_empty() { None } else { Some(text) }, None);
    }
    let (left, right) = raw.split_once("||").unwrap_or((&raw, ""));
    let copyright = left.trim();
    let url = right.trim();
    (
        if copyright.is_empty() { None } else { Some(copyright.to_string()) },
        if url.is_empty() { None } else { Some(url.to_string()) },
    )
}

fn meta_from_jpeg(bytes: &[u8]) -> Option<WordMeta> {
    let exif = Reader::new().read_from_container(&mut Cursor::new(bytes)).ok()?;
    let (copyright, copyright_url) = split_copyright(field_text(&exif, Tag::Copyright));
    Some(WordMeta {
        word: field_text(&exif, Tag::Artist),
        dictionary_url: field_text(&exif, Tag::ImageDescription),
        audio_url: field_text(&exif, Tag(Context::Tiff, 0x010d)),
        copyright,
        copyright_url,
        image_id: field_text(&exif, Tag::Software),
    })
}

pub async fn fetch_word_meta(random: bool) -> Result<WordMeta, String> {
    let mut url = "https://ss.blueforge.org/bing?v=2.0.0&w=64&h=36".to_string();
    if random {
        url.push_str("&random");
    }
    let response = client()
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|err| format!("单词下载失败（{err}）"))?;
    if !response.status().is_success() {
        return Err(format!("单词下载失败（HTTP {}）", response.status()));
    }
    let mut buf = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| format!("单词下载失败（{err}）"))?;
        buf.extend_from_slice(&chunk);
        if let Some(meta) = meta_from_jpeg(&buf) {
            if meta.word.is_some() || meta.image_id.is_some() {
                if meta.word.is_some() {
                    return Ok(meta);
                }
            }
        }
        if buf.len() >= 512 * 1024 {
            break;
        }
    }
    let meta = meta_from_jpeg(&buf).unwrap_or_default();
    if meta.word.is_none() {
        return Err("没有拿到单词".into());
    }
    Ok(meta)
}

fn decode_piece(raw: &str) -> Vec<String> {
    let with_breaks = Regex::new(r"(?i)<br\s*/?>")
        .unwrap()
        .replace_all(raw, "\n");
    let stripped = Regex::new(r"(?is)<[^>]+>")
        .unwrap()
        .replace_all(&with_breaks, "");
    let decoded = Regex::new(r"&#(\d+);")
        .unwrap()
        .replace_all(&stripped, |caps: &regex::Captures| {
            let code = caps[1].parse::<u32>().unwrap_or(0);
            char::from_u32(code).map(|ch| ch.to_string()).unwrap_or_default()
        });
    decoded
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .split('\n')
        .map(|line| {
            line.replace('🔊', "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|line| !line.is_empty())
        .collect()
}

pub async fn fetch_caption(image_id: &str) -> Vec<String> {
    if image_id.is_empty() {
        return Vec::new();
    }
    let url = format!("https://ss.blueforge.org/bing/s/{image_id}.htm");
    let Ok(response) = client().get(url).timeout(Duration::from_secs(20)).send().await else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(html) = response.text().await else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if let Some(found) = Regex::new(r"(?is)<h1[^>]*>(.*?)</h1>").unwrap().captures(&html) {
        lines.extend(decode_piece(&found[1]));
    }
    if let Some(found) = Regex::new(r"(?is)<h3[^>]*>(.*?)</h3>").unwrap().captures(&html) {
        lines.extend(decode_piece(&found[1]));
    }
    lines.truncate(4);
    lines
}

fn absolute_bing(url: &str) -> String {
    if url.starts_with("http") {
        url.to_string()
    } else {
        format!("https://www.bing.com{url}")
    }
}

#[derive(Deserialize)]
struct BingArchive {
    images: Option<Vec<BingEntry>>,
}

#[derive(Deserialize)]
struct BingEntry {
    copyright: Option<String>,
    url: Option<String>,
}

pub async fn list_recent_bing(count: u8) -> Vec<BingImage> {
    let url = format!("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n={count}&mkt=zh-CN");
    let Ok(response) = client().get(url).timeout(Duration::from_secs(8)).send().await else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(info) = response.json::<BingArchive>().await else {
        return Vec::new();
    };
    info.images
        .unwrap_or_default()
        .into_iter()
        .filter_map(|image| {
            let url = image.url?;
            Some(BingImage {
                copyright: image.copyright.unwrap_or_default(),
                url: absolute_bing(&url),
            })
        })
        .collect()
}

pub fn same_place(left: &str, right: &str) -> bool {
    let a: String = left.chars().filter(|ch| !ch.is_whitespace()).collect();
    let b: String = right.chars().filter(|ch| !ch.is_whitespace()).collect();
    if a.chars().count() < 6 || b.chars().count() < 6 {
        return false;
    }
    let a12: String = a.chars().take(12).collect();
    let b12: String = b.chars().take(12).collect();
    a.contains(&b12) || b.contains(&a12)
}

pub fn find_clean<'a>(images: &'a [BingImage], copyright: &str, today: bool) -> Option<&'a str> {
    let matched = images.iter().find(|image| same_place(copyright, &image.copyright));
    let chosen = matched.or(if today { images.first() } else { None });
    chosen.map(|image| image.url.as_str())
}

pub fn pick_recent(images: &[BingImage], copyright: &str, avoid_copyright: &str, avoid_url: &str) -> Option<String> {
    if images.is_empty() {
        return None;
    }
    let candidates: Vec<&BingImage> = images
        .iter()
        .filter(|image| avoid_url.is_empty() || image.url != avoid_url)
        .collect();
    let candidates = if candidates.is_empty() { images.iter().collect() } else { candidates };
    if let Some(matched) = candidates
        .iter()
        .find(|image| same_place(&image.copyright, copyright) && !same_place(&image.copyright, avoid_copyright))
    {
        return Some(matched.url.clone());
    }
    let others: Vec<&BingImage> = candidates
        .iter()
        .copied()
        .filter(|image| !same_place(&image.copyright, avoid_copyright))
        .collect();
    let pool = if others.is_empty() { candidates } else { others };
    // macOS 时钟只到微秒，subsec_nanos 总是 1000 的倍数，对 8 取模恒为 0，会一直选中今天。
    let tick = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_micros() as usize)
        .unwrap_or(0);
    Some(pool[tick % pool.len()].url.clone())
}

pub async fn download_bytes(url: &str, timeout_secs: u64) -> Result<Vec<u8>, String> {
    let response = client()
        .get(url)
        .timeout(Duration::from_secs(timeout_secs))
        .send()
        .await
        .map_err(|err| format!("图片下载失败（{err}）"))?;
    if !response.status().is_success() {
        return Err(format!("图片下载失败（HTTP {}）", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("图片下载失败（{err}）"))?
        .to_vec();
    if bytes.len() < 1000 {
        return Err("图片下载失败（内容为空）".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{decode_bytes, join_decoded, pick_recent, restore_exif_text, BingImage};

    fn sample() -> Vec<BingImage> {
        (0..8)
            .map(|index| BingImage {
                copyright: format!("地点{index}的一段足够长的说明文字"),
                url: format!("https://www.bing.com/{index}.jpg"),
            })
            .collect()
    }

    #[test]
    fn past_wallpaper_skips_the_current_photo() {
        let images = sample();
        for _ in 0..30 {
            let url = pick_recent(&images, "另一处完全不同的风景名称说明", "也不相同的旧地点说明文字", &images[0].url)
                .expect("有其它壁纸");
            assert_ne!(url, images[0].url);
        }
    }

    #[test]
    fn copyright_utf8_is_not_escaped() {
        let raw = "粉红色的苹果花, 阿维拉海滩, 加利福尼亚州, 美国 (© Mimi Ditchie Photography/Getty Images)";
        assert_eq!(decode_bytes(raw.as_bytes()), raw);
        assert_eq!(join_decoded(&[raw.as_bytes().to_vec()]), raw);
        let escaped = r"\xe7\xb2\x89\xe7\xba\xa2\xe8\x89\xb2\xe7\x9a\x84\xe8\x8b\xb9\xe6\x9e\x9c\xe8\x8a\xb1, \xe9\x98\xbf\xe7\xbb\xb4\xe6\x8b\x89\xe6\xb5\xb7\xe6\xbb\xa9, \xe5\x8a\xa0\xe5\x88\xa9\xe7\xa6\x8f\xe5\xb0\xbc\xe4\xba\x9a\xe5\xb7\x9e, \xe7\xbe\x8e\xe5\x9b\xbd (\xc2\xa9 Mimi Ditchie Photography/Getty Images)";
        assert_eq!(restore_exif_text(escaped), raw);
        assert_eq!(restore_exif_text("scarce"), "scarce");
    }
}
