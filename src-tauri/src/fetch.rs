use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Clone, Debug, Default)]
pub struct LevelWord {
    pub word: String,
    pub dictionary_url: String,
    pub audio_url: String,
    pub lines: Vec<String>,
}

/// 填字游戏用的一条词目：单词 + 音标 + 中文释义行 + 词典/发音地址。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizWord {
    pub word: String,
    pub phone: String,
    pub lines: Vec<String>,
    pub dictionary_url: String,
    pub audio_url: String,
}

#[derive(Clone, Debug)]
pub struct Wallpaper {
    pub url: String,
    pub copyright: String,
}

#[derive(Deserialize)]
struct QwertyWord {
    name: String,
    #[serde(default)]
    trans: Vec<String>,
    #[serde(default)]
    usphone: Option<String>,
    #[serde(default)]
    ukphone: Option<String>,
}

#[derive(Deserialize)]
struct YoudaoResp {
    blng_sents_part: Option<BlngPart>,
    auth_sents_part: Option<AuthPart>,
}

#[derive(Deserialize)]
struct BlngPart {
    #[serde(default, rename = "sentence-pair")]
    pairs: Vec<SentencePair>,
}

#[derive(Deserialize)]
struct SentencePair {
    sentence: Option<String>,
    #[serde(rename = "sentence-translation")]
    translation: Option<String>,
}

#[derive(Deserialize)]
struct AuthPart {
    #[serde(default)]
    sent: Vec<AuthSent>,
}

#[derive(Deserialize)]
struct AuthSent {
    foreign: Option<String>,
}

fn strip_tags(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn now_micros() -> u64 {
    // macOS 时钟只到微秒，subsec_nanos 总是 1000 的倍数，对 8 取模恒为 0，会一直选中今天。
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_micros() as u64)
        .unwrap_or(0)
}

fn level_file(level: &str) -> &'static str {
    match level {
        "cet6" => "CET6_T.json",
        "kaoyan" => "KaoYan_3_T.json",
        "ielts" => "IELTS_3_T.json",
        "toefl" => "TOEFL_3_T.json",
        "gre" => "GRE_3_T.json",
        _ => "CET4_T.json",
    }
}

/// 用有道的免费接口给单词取一句双语例句，返回 [英文, 中文]（拿不到中文时只返回英文）。
pub async fn fetch_example(word: &str) -> Vec<String> {
    let url = format!("https://dict.youdao.com/jsonapi?q={word}");
    let Ok(response) = client().get(url).timeout(Duration::from_secs(10)).send().await else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(data) = response.json::<YoudaoResp>().await else {
        return Vec::new();
    };
    if let Some(part) = data.blng_sents_part {
        for pair in part.pairs {
            let en = strip_tags(&pair.sentence.unwrap_or_default());
            if !en.is_empty() {
                let cn = strip_tags(&pair.translation.unwrap_or_default());
                return if cn.is_empty() { vec![en] } else { vec![en, cn] };
            }
        }
    }
    if let Some(part) = data.auth_sents_part {
        for sent in part.sent {
            let en = strip_tags(&sent.foreign.unwrap_or_default());
            if !en.is_empty() {
                return vec![en];
            }
        }
    }
    Vec::new()
}

/// 从 qwerty-learner 的分级词库里随机取一个词，返回单词、词典地址、发音地址和释义行。
pub async fn fetch_level_word(level: &str) -> Result<LevelWord, String> {
    let url = format!(
        "https://cdn.jsdelivr.net/gh/Kaiyiwing/qwerty-learner@master/public/dicts/{}",
        level_file(level)
    );
    let list: Vec<QwertyWord> = client()
        .get(url)
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .map_err(|err| format!("词库下载失败（{err}）"))?
        .json()
        .await
        .map_err(|err| format!("词库解析失败（{err}）"))?;
    if list.is_empty() {
        return Err("词库是空的".into());
    }
    let word = &list[now_micros() as usize % list.len()];
    let phone = word
        .usphone
        .clone()
        .or_else(|| word.ukphone.clone())
        .map(|value| value.trim().trim_matches('/').to_string())
        .unwrap_or_default();
    let title = if phone.is_empty() {
        word.name.clone()
    } else {
        format!("{}  /{}/", word.name, phone)
    };
    let mut lines = vec![title];
    for text in word.trans.iter().take(2) {
        let text = text.trim();
        if !text.is_empty() {
            lines.push(text.to_string());
        }
    }
    lines.extend(fetch_example(&word.name).await);
    lines.truncate(5);
    Ok(LevelWord {
        word: word.name.clone(),
        dictionary_url: format!("https://dict.youdao.com/w/eng/{}/", word.name),
        audio_url: format!("https://dict.youdao.com/dictvoice?audio={}&type=2", word.name),
        lines,
    })
}

fn to_quiz_word(word: &QwertyWord) -> QuizWord {
    let phone = word
        .usphone
        .clone()
        .or_else(|| word.ukphone.clone())
        .map(|value| value.trim().trim_matches('/').to_string())
        .unwrap_or_default();
    QuizWord {
        word: word.name.clone(),
        phone,
        lines: word
            .trans
            .iter()
            .take(2)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect(),
        dictionary_url: format!("https://dict.youdao.com/w/eng/{}/", word.name),
        audio_url: format!("https://dict.youdao.com/dictvoice?audio={}&type=2", word.name),
    }
}

/// 给填字游戏抽词：整表下载一次，随机抽 count 个不重复的条目（不含短语）。
pub async fn fetch_quiz_words(level: &str, count: usize) -> Result<Vec<QuizWord>, String> {
    let url = format!(
        "https://cdn.jsdelivr.net/gh/Kaiyiwing/qwerty-learner@master/public/dicts/{}",
        level_file(level)
    );
    let list: Vec<QwertyWord> = client()
        .get(url)
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .map_err(|err| format!("词库下载失败（{err}）"))?
        .json()
        .await
        .map_err(|err| format!("词库解析失败（{err}）"))?;
    // 填字要求一个词一串字母：留 3~12 个纯小写字母、有中文释义的条目。
    let candidates: Vec<&QwertyWord> = list
        .iter()
        .filter(|word| {
            (3..=12).contains(&word.name.len())
                && word.name.chars().all(|ch| ch.is_ascii_lowercase())
                && word.trans.iter().any(|text| !text.trim().is_empty())
        })
        .collect();
    if candidates.is_empty() {
        return Err("词库里没有合适的单词".into());
    }
    let mut picked: Vec<QuizWord> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for attempt in 0..4000u64 {
        if picked.len() >= count || picked.len() >= candidates.len() {
            break;
        }
        // 同一微秒内连抽会撞同一个下标，掺进进程号和次数打散。
        let seed = now_micros() ^ ((attempt + 1) << 23) ^ (std::process::id() as u64);
        let word = candidates[seed as usize % candidates.len()];
        if seen.insert(word.name.clone()) {
            picked.push(to_quiz_word(word));
        }
    }
    if picked.is_empty() {
        return Err("没有凑出单词".into());
    }
    Ok(picked)
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

/// Bing 每日壁纸，免 key。`count` 越大越能拿到前几天。
pub async fn list_bing(count: u8) -> Vec<Wallpaper> {
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
            Some(Wallpaper {
                copyright: image.copyright.unwrap_or_default(),
                url: absolute_bing(&url),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct WallhavenResp {
    data: Option<Vec<WallhavenItem>>,
}

#[derive(Deserialize)]
struct WallhavenItem {
    path: Option<String>,
    uploader: Option<WallhavenUploader>,
}

#[derive(Deserialize)]
struct WallhavenUploader {
    username: Option<String>,
}

/// Wallhaven 壁纸站，免 key（purity=100 仅安全图）。
pub async fn list_wallhaven(count: u8) -> Vec<Wallpaper> {
    let seed = format!("{:x}", now_micros());
    let seed = &seed[seed.len().saturating_sub(6)..];
    let url = format!(
        "https://wallhaven.cc/api/v1/search?categories=111&purity=100&ratios=16x9&sorting=random&atleast=1920x1080&per_page={}&seed={seed}",
        count.clamp(1, 24)
    );
    let Ok(response) = client().get(url).timeout(Duration::from_secs(12)).send().await else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(info) = response.json::<WallhavenResp>().await else {
        return Vec::new();
    };
    info.data
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let path = item.path?;
            let uploader = item.uploader.and_then(|value| value.username).unwrap_or_default();
            Some(Wallpaper {
                url: path,
                copyright: if uploader.is_empty() {
                    "Wallhaven".into()
                } else {
                    format!("Wallhaven · {uploader}")
                },
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct PicsumItem {
    id: Option<String>,
    author: Option<String>,
}

/// Lorem Picsum 随机摄影图，免 key。
pub async fn list_picsum(count: u8) -> Vec<Wallpaper> {
    let page = (now_micros() % 30) + 1;
    let url = format!("https://picsum.photos/v2/list?page={page}&limit={}", count.clamp(1, 100));
    let Ok(response) = client().get(url).timeout(Duration::from_secs(10)).send().await else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(list) = response.json::<Vec<PicsumItem>>().await else {
        return Vec::new();
    };
    list.into_iter()
        .filter_map(|item| {
            let id = item.id?;
            Some(Wallpaper {
                url: format!("https://picsum.photos/id/{id}/2560/1440"),
                copyright: item.author.unwrap_or_default(),
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

pub fn pick_recent(images: &[Wallpaper], copyright: &str, avoid_copyright: &str, avoid_url: &str) -> Option<String> {
    if images.is_empty() {
        return None;
    }
    let candidates: Vec<&Wallpaper> = images
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
    let others: Vec<&Wallpaper> = candidates
        .iter()
        .copied()
        .filter(|image| !same_place(&image.copyright, avoid_copyright))
        .collect();
    let pool = if others.is_empty() { candidates } else { others };
    Some(pool[now_micros() as usize % pool.len()].url.clone())
}

pub fn find_clean<'a>(images: &'a [Wallpaper], copyright: &str, today: bool) -> Option<&'a str> {
    let matched = images.iter().find(|image| same_place(copyright, &image.copyright));
    let chosen = matched.or(if today { images.first() } else { None });
    chosen.map(|image| image.url.as_str())
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

/// 旧版把壁纸来源写进 EXIF，非 ASCII 会被转义。读取历史数据时还原一下。
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

#[cfg(test)]
mod tests {
    use super::{pick_recent, restore_exif_text, Wallpaper};

    fn sample() -> Vec<Wallpaper> {
        (0..8)
            .map(|index| Wallpaper {
                copyright: format!("地点{index}的一段足够长的说明文字"),
                url: format!("https://example.com/{index}.jpg"),
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
    fn copyright_utf8_is_restored() {
        let raw = "粉红色的苹果花, 阿维拉海滩, 加利福尼亚州, 美国";
        let escaped = r"\xe7\xb2\x89\xe7\xba\xa2\xe8\x89\xb2\xe7\x9a\x84\xe8\x8b\xb9\xe6\x9e\x9c\xe8\x8a\xb1, \xe9\x98\xbf\xe7\xbb\xb4\xe6\x8b\x89\xe6\xb5\xb7\xe6\xbb\xa9, \xe5\x8a\xa0\xe5\x88\xa9\xe7\xa6\x8f\xe5\xb0\xbc\xe4\xba\x9a\xe5\xb7\x9e, \xe7\xbe\x8e\xe5\x9b\xbd";
        assert_eq!(restore_exif_text(escaped), raw);
        assert_eq!(restore_exif_text("scarlet"), "scarlet");
    }
}
