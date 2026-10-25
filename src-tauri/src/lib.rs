mod fetch;
mod image_ops;

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::Mutex;

static QUITTING: AtomicBool = AtomicBool::new(false);

const CLIP_LABEL: &str = "clip";
// 和 .wallish-clip__bar 的 height 对齐，子 webview 从这条下面开始铺。
const CLIP_BAR_PX: f64 = 52.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WordBox {
    position: String,
    background: String,
    size: String,
    caption: bool,
}

impl Default for WordBox {
    fn default() -> Self {
        Self {
            position: "bottom".into(),
            background: "#f4efe6".into(),
            size: "medium".into(),
            caption: true,
        }
    }
}

fn default_level() -> String {
    "cet4".into()
}

fn normalize_level(level: &str) -> String {
    match level {
        "cet4" | "cet6" | "kaoyan" | "ielts" | "toefl" | "gre" | "jp-n5" | "jp-n4" | "jp-n3" | "jp-n2" | "jp-n1" => {
            level.to_string()
        }
        _ => default_level(),
    }
}

fn default_source() -> String {
    "auto".into()
}

fn normalize_source(source: &str) -> String {
    match source {
        "auto" | "photo" | "anime" | "wallhaven" | "bing" | "picsum" => source.to_string(),
        _ => default_source(),
    }
}

/// 判断某个下载地址是否更适合走代理（离线补下载时用，避免先干等直连超时）。
fn url_prefers_proxy(url: &str) -> bool {
    url.contains("wallhaven")
}

fn normalize_word_box(value: &serde_json::Value) -> WordBox {
    let mut box_ = WordBox::default();
    if let Some(position) = value.get("position").and_then(|item| item.as_str()) {
        let position = match position {
            "top-left" | "top-right" => "top",
            "bottom-left" | "bottom-right" => "bottom",
            other => other,
        };
        if matches!(position, "top" | "center" | "bottom" | "left" | "right") {
            box_.position = position.into();
        }
    }
    if let Some(background) = value.get("background").and_then(|item| item.as_str()) {
        if background.len() == 7 && background.starts_with('#') && background[1..].chars().all(|ch| ch.is_ascii_hexdigit()) {
            // 旧版默认的深色底换成纸色，用户自己挑的颜色保持不变
            box_.background = if background.eq_ignore_ascii_case("#1f2a36") {
                WordBox::default().background
            } else {
                background.to_lowercase()
            };
        }
    }
    if let Some(size) = value.get("size").and_then(|item| item.as_str()) {
        if matches!(size, "small" | "medium" | "large") {
            box_.size = size.into();
        }
    }
    box_.caption = value.get("caption").and_then(|item| item.as_bool()).unwrap_or(true);
    box_
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedState {
    word: Option<String>,
    dictionary_url: Option<String>,
    audio_url: Option<String>,
    copyright: Option<String>,
    copyright_url: Option<String>,
    image_id: Option<String>,
    image_path: Option<String>,
    caption_lines: Option<Vec<String>>,
    source: String,
    favorite_id: Option<String>,
    word_box: WordBox,
    random: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    photo_url: Option<String>,
    #[serde(default = "default_level")]
    level: String,
    #[serde(default = "default_source")]
    wallpaper_source: String,
}

impl Default for SavedState {
    fn default() -> Self {
        Self {
            word: None,
            dictionary_url: None,
            audio_url: None,
            copyright: None,
            copyright_url: None,
            image_id: None,
            image_path: None,
            caption_lines: None,
            source: "bing".into(),
            favorite_id: None,
            word_box: WordBox::default(),
            random: false,
            error: None,
            photo_url: None,
            level: default_level(),
            wallpaper_source: default_source(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewState {
    word: Option<String>,
    dictionary_url: Option<String>,
    audio_url: Option<String>,
    copyright: Option<String>,
    image_data_url: Option<String>,
    caption_lines: Option<Vec<String>>,
    word_box: WordBox,
    favorited: bool,
    error: Option<String>,
    level: String,
    wallpaper_source: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Plate {
    view: ViewState,
    paint: bool,
    clean_base64: String,
    lines: Vec<String>,
    word_box: WordBox,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FavoriteItem {
    id: String,
    word: String,
    copyright: String,
    source: String,
    thumb_url: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FavoriteRecord {
    id: String,
    word: Option<String>,
    dictionary_url: Option<String>,
    audio_url: Option<String>,
    copyright: Option<String>,
    copyright_url: Option<String>,
    image_id: Option<String>,
    caption_lines: Option<Vec<String>>,
    source: String,
    word_box: WordBox,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomPreview {
    name: String,
    image_data_url: String,
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
}

struct PendingCustom {
    path: PathBuf,
    name: String,
    target_width: u32,
    target_height: u32,
}

struct HomeBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

struct Core {
    saved: SavedState,
    pending: Option<PendingCustom>,
    home: Option<HomeBounds>,
}

struct AppState {
    core: Mutex<Core>,
}

fn data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn decode_jpeg(value: &str) -> Result<Vec<u8>, String> {
    let payload = value.split(',').next_back().unwrap_or(value);
    base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|_| "壁纸图片无法打开".to_string())
}

fn app_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

fn read_saved(dir: &Path) -> SavedState {
    let path = dir.join("state.json");
    let Ok(text) = fs::read_to_string(path) else {
        return SavedState::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return SavedState::default();
    };
    let mut saved = serde_json::from_value::<SavedState>(value.clone()).unwrap_or_default();
    saved.word_box = normalize_word_box(value.get("wordBox").unwrap_or(&serde_json::Value::Null));
    saved.copyright = saved.copyright.map(|text| fetch::restore_exif_text(&text));
    saved
}

fn write_saved(dir: &Path, saved: &SavedState) -> Result<(), String> {
    let text = serde_json::to_string_pretty(saved).map_err(|err| err.to_string())?;
    fs::write(dir.join("state.json"), text).map_err(|err| err.to_string())
}

fn favorites_path(dir: &Path) -> PathBuf {
    dir.join("favorites.json")
}

fn favorites_dir(dir: &Path) -> PathBuf {
    dir.join("favorites")
}

fn load_favorites(dir: &Path) -> Vec<FavoriteRecord> {
    let Ok(text) = fs::read_to_string(favorites_path(dir)) else {
        return Vec::new();
    };
    let mut items: Vec<FavoriteRecord> = serde_json::from_str(&text).unwrap_or_default();
    for item in &mut items {
        item.copyright = item.copyright.as_deref().map(fetch::restore_exif_text);
    }
    items
}

fn save_favorites(dir: &Path, items: &[FavoriteRecord]) -> Result<(), String> {
    fs::write(favorites_path(dir), serde_json::to_string(items).map_err(|err| err.to_string())?).map_err(|err| err.to_string())
}

fn current_favorite<'a>(saved: &SavedState, items: &'a [FavoriteRecord]) -> Option<&'a FavoriteRecord> {
    if let Some(id) = &saved.favorite_id {
        if let Some(hit) = items.iter().find(|item| &item.id == id) {
            return Some(hit);
        }
    }
    if saved.source != "custom" {
        if let Some(image_id) = &saved.image_id {
            return items
                .iter()
                .find(|item| item.source != "custom" && item.image_id.as_deref() == Some(image_id));
        }
    }
    None
}

fn preview_file(dir: &Path, saved: &SavedState) -> Option<PathBuf> {
    let composed = dir.join("wallpaper-composed.jpg");
    if composed.exists() {
        return Some(composed);
    }
    saved.image_path.as_ref().map(PathBuf::from).filter(|path| path.exists())
}

fn view_of(dir: &Path, saved: &SavedState) -> ViewState {
    let items = load_favorites(dir);
    let image_data_url = preview_file(dir, saved).and_then(|path| fs::read(path).ok()).map(|bytes| data_url(&bytes));
    ViewState {
        word: saved.word.clone(),
        dictionary_url: saved.dictionary_url.clone(),
        audio_url: saved.audio_url.clone(),
        copyright: saved.copyright.clone(),
        image_data_url,
        caption_lines: saved.caption_lines.clone(),
        word_box: saved.word_box.clone(),
        favorited: current_favorite(saved, &items).is_some(),
        error: saved.error.clone(),
        level: saved.level.clone(),
        wallpaper_source: saved.wallpaper_source.clone(),
    }
}

fn empty_plate(view: ViewState) -> Plate {
    let word_box = view.word_box.clone();
    Plate {
        view,
        paint: false,
        clean_base64: String::new(),
        lines: Vec::new(),
        word_box,
    }
}

fn wallpaper_size(window: &tauri::Window) -> (u32, u32) {
    let Some(monitor) = window.primary_monitor().ok().flatten() else {
        return (1920, 1080);
    };
    let size = monitor.size();
    (size.width.max(1280), size.height.max(720))
}

fn set_mac_wallpaper(dir: &Path, image: &Path) -> Result<PathBuf, String> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_millis()).unwrap_or(0);
    let shown = dir.join(format!("wallpaper_show_{millis}.jpg"));
    fs::copy(image, &shown).map_err(|err| err.to_string())?;
    set_desktop_wallpaper(&shown)?;
    if let Ok(names) = fs::read_dir(dir) {
        for name in names.flatten() {
            let path = name.path();
            let file_name = path.file_name().and_then(|item| item.to_str()).unwrap_or("");
            if file_name.starts_with("wallpaper_show_") && path != shown {
                let _ = fs::remove_file(path);
            }
        }
    }
    Ok(shown)
}

#[cfg(target_os = "macos")]
fn set_desktop_wallpaper(shown: &Path) -> Result<(), String> {
    let script = format!(
        "tell application \"System Events\" to set picture of every desktop to POSIX file \"{}\"",
        shown.display().to_string().replace('"', "\\\"")
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() { "设置桌面失败".into() } else { detail });
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_desktop_wallpaper(shown: &Path) -> Result<(), String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(
            ui_action: u32,
            ui_param: u32,
            pv_param: *mut c_void,
            ui_win_ini: u32,
        ) -> i32;
    }

    // SPI_SETDESKWALLPAPER 会把 .jpg/.bmp 等位图设为桌面背景，Windows 10/11 均支持 jpg。
    const SPI_SETDESKWALLPAPER: u32 = 20;
    // UPDATEINIFILE 写注册表持久化，SENDCHANGE 广播 WM_SETTINGCHANGE 让桌面立即刷新。
    const SPIF_UPDATEINIFILE: u32 = 0x01;
    const SPIF_SENDCHANGE: u32 = 0x02;

    let wide: Vec<u16> = std::ffi::OsStr::new(shown)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            wide.as_ptr() as *mut c_void,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    };
    if ok == 0 {
        Err("设置桌面失败".into())
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn set_desktop_wallpaper(_shown: &Path) -> Result<(), String> {
    Err("这版试用只会在 macOS 和 Windows 上更换桌面壁纸".into())
}

fn plate_from_clean(dir: &Path, saved: &SavedState, clean: &Path) -> Result<Plate, String> {
    let lines = if saved.caption_lines.as_ref().map(|items| !items.is_empty()).unwrap_or(false) {
        saved.caption_lines.clone().unwrap_or_default()
    } else if let Some(word) = &saved.word {
        vec![word.clone()]
    } else {
        Vec::new()
    };
    let paint = saved.word_box.caption && !lines.is_empty();
    if !paint {
        let composed = dir.join("wallpaper-composed.jpg");
        fs::copy(clean, &composed).map_err(|err| err.to_string())?;
        set_mac_wallpaper(dir, &composed)?;
        return Ok(empty_plate(view_of(dir, saved)));
    }
    let bytes = fs::read(clean).map_err(|err| err.to_string())?;
    Ok(Plate {
        view: view_of(dir, saved),
        paint: true,
        clean_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        lines,
        word_box: saved.word_box.clone(),
    })
}

async fn ensure_caption(saved: &mut SavedState) -> Vec<String> {
    if let Some(lines) = &saved.caption_lines {
        if !lines.is_empty() {
            return lines.clone();
        }
    }
    let lines = saved.word.clone().map(|word| vec![word]).unwrap_or_default();
    saved.caption_lines = Some(lines.clone());
    lines
}

async fn ensure_clean(dir: &Path, saved: &SavedState) -> Result<PathBuf, String> {
    if saved.source == "custom" {
        let custom = dir.join("wallpaper-custom.jpg");
        if !custom.exists() {
            return Err("自定义图片不见了，请重新选择".into());
        }
        return Ok(custom);
    }
    let marker_id = saved
        .photo_url
        .clone()
        .or_else(|| saved.image_id.clone())
        .unwrap_or_default();
    if marker_id.is_empty() {
        return Err("这张壁纸没有原图".into());
    }
    let clean = dir.join("wallpaper-clean.jpg");
    let marker_path = dir.join("wallpaper-clean.txt");
    let marker = fs::read_to_string(&marker_path).unwrap_or_default();
    if marker == format!("bing:{marker_id}") && clean.exists() {
        return Ok(clean);
    }
    let url = if marker_id.starts_with("http") {
        marker_id.clone()
    } else {
        let images = fetch::list_bing(if saved.random { 8 } else { 1 }).await;
        fetch::find_clean(&images, saved.copyright.as_deref().unwrap_or(""), !saved.random)
            .ok_or_else(|| "这张壁纸不在最近 8 天里".to_string())?
            .to_string()
    };
    let bytes = fetch::download_image(&url, 25, url_prefers_proxy(&url)).await?;
    let part = dir.join("wallpaper-clean.jpg.part");
    fs::write(&part, &bytes).map_err(|err| err.to_string())?;
    fs::rename(&part, &clean).map_err(|err| err.to_string())?;
    fs::write(&marker_path, format!("bing:{marker_id}")).map_err(|err| err.to_string())?;
    Ok(clean)
}

#[tauri::command]
async fn get_state(app: AppHandle, state: State<'_, AppState>) -> Result<ViewState, String> {
    let dir = app_dir(&app)?;
    let core = state.core.lock().await;
    Ok(view_of(&dir, &core.saved))
}

async fn fetch_past_photo(
    images: &[fetch::Wallpaper],
    copyright: &str,
    avoid_copyright: &str,
    avoid_url: &str,
    previous: &[u8],
    prefer_proxy: bool,
) -> Result<(String, String, Vec<u8>), String> {
    let first = fetch::pick_recent(images, copyright, avoid_copyright, avoid_url)
        .ok_or_else(|| "没有可用的壁纸".to_string())?;
    let start = images.iter().position(|image| image.url == first).unwrap_or(0);
    let mut fallback: Option<(String, String, Vec<u8>)> = None;
    for offset in 0..images.len() {
        let image = &images[(start + offset) % images.len()];
        if !avoid_url.is_empty() && image.url == avoid_url {
            continue;
        }
        let Ok(bytes) = fetch::download_image(&image.url, 30, prefer_proxy).await else {
            continue;
        };
        if fallback.is_none() {
            fallback = Some((image.url.clone(), image.copyright.clone(), bytes.clone()));
        }
        if previous.is_empty() || bytes != previous {
            return Ok((image.url.clone(), image.copyright.clone(), bytes));
        }
    }
    fallback.ok_or_else(|| "没有可用的壁纸".to_string())
}

async fn try_provider(
    provider: &str,
    random: bool,
    avoid_copyright: &str,
    avoid_url: &str,
    previous: &[u8],
) -> Result<(String, String, Vec<u8>), String> {
    match provider {
        "bing" => {
            if random {
                let images = fetch::list_bing(8).await;
                if images.is_empty() {
                    return Err("Bing 没有返回壁纸".into());
                }
                fetch_past_photo(&images, "", avoid_copyright, avoid_url, previous, false).await
            } else {
                let images = fetch::list_bing(1).await;
                let image = images.first().ok_or_else(|| "Bing 没有返回壁纸".to_string())?;
                let bytes = fetch::download_image(&image.url, 15, false).await?;
                Ok((image.url.clone(), image.copyright.clone(), bytes))
            }
        }
        "wallhaven" => {
            let images = fetch::list_wallhaven(24).await;
            if images.is_empty() {
                return Err("Wallhaven 没有返回壁纸".into());
            }
            fetch_past_photo(&images, "", avoid_copyright, avoid_url, previous, true).await
        }
        "photo" => {
            // alcy 的 /fj 是风景摄影；/pc 是混合 PC 壁纸(含二次元/游戏)，不能拿来当"风景图"。
            const ENDPOINTS: [&str; 1] = ["https://t.alcy.cc/fj"];
            for endpoint in ENDPOINTS {
                if let Ok(bytes) = fetch::download_image(endpoint, 15, false).await {
                    return Ok((endpoint.to_string(), "风景图 · alcy.cc".into(), bytes));
                }
            }
            Err("风景图源没有返回壁纸".into())
        }
        "anime" => {
            match fetch::download_image("https://www.loliapi.com/acg/", 15, false).await {
                Ok(bytes) => Ok(("https://www.loliapi.com/acg/".to_string(), "二次元 · loliapi".into(), bytes)),
                Err(_) => Err("二次元源没有返回壁纸".into()),
            }
        }
        "picsum" => {
            let images = fetch::list_picsum(30).await;
            if images.is_empty() {
                return Err("Picsum 没有返回壁纸".into());
            }
            fetch_past_photo(&images, "", avoid_copyright, avoid_url, previous, false).await
        }
        _ => Err("未知的壁纸来源".into()),
    }
}

async fn pick_wallpaper(
    source: &str,
    random: bool,
    avoid_copyright: &str,
    avoid_url: &str,
    previous: &[u8],
) -> Result<(String, String, Vec<u8>), String> {
    let providers: &[&str] = match source {
        "photo" => &["photo"],
        "anime" => &["anime"],
        "bing" => &["bing"],
        "wallhaven" => &["wallhaven"],
        "picsum" => &["picsum"],
        _ => &["bing", "photo", "anime", "wallhaven", "picsum"],
    };
    let mut first_error = None;
    for provider in providers {
        match try_provider(provider, random, avoid_copyright, avoid_url, previous).await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }
    }
    Err(first_error.unwrap_or_else(|| "没有拿到壁纸".into()))
}

#[tauri::command]
async fn switch_wallpaper(app: AppHandle, state: State<'_, AppState>, random: bool) -> Result<Plate, String> {
    let dir = app_dir(&app)?;
    let mut core = state.core.lock().await;
    let level = core.saved.level.clone();
    let wallpaper_source = normalize_source(&core.saved.wallpaper_source);
    let word = fetch::fetch_level_word(&level).await?;
    let avoid_copyright = core.saved.copyright.clone().unwrap_or_default();
    let avoid_url = core.saved.photo_url.clone().unwrap_or_default();
    let previous = fs::read(dir.join("wallpaper-clean.jpg")).unwrap_or_default();
    let (photo_url, copyright, bytes) =
        pick_wallpaper(&wallpaper_source, random, &avoid_copyright, &avoid_url, &previous).await?;
    let clean = dir.join("wallpaper-clean.jpg");
    let part = dir.join("wallpaper-clean.jpg.part");
    fs::write(&part, &bytes).map_err(|err| err.to_string())?;
    fs::rename(&part, &clean).map_err(|err| err.to_string())?;
    fs::write(dir.join("wallpaper-clean.txt"), format!("bing:{photo_url}")).map_err(|err| err.to_string())?;
    let word_box = core.saved.word_box.clone();
    core.saved = SavedState {
        word: Some(word.word),
        dictionary_url: Some(word.dictionary_url),
        audio_url: Some(word.audio_url),
        copyright: Some(copyright),
        copyright_url: None,
        image_id: Some(photo_url.clone()),
        image_path: Some(clean.display().to_string()),
        caption_lines: Some(word.lines),
        source: "bing".into(),
        favorite_id: None,
        word_box,
        random,
        error: None,
        photo_url: Some(photo_url),
        level,
        wallpaper_source,
    };
    write_saved(&dir, &core.saved)?;
    plate_from_clean(&dir, &core.saved, &clean)
}

#[tauri::command]
async fn set_wallpaper_source(app: AppHandle, state: State<'_, AppState>, source: String) -> Result<ViewState, String> {
    let dir = app_dir(&app)?;
    let mut core = state.core.lock().await;
    core.saved.wallpaper_source = normalize_source(&source);
    write_saved(&dir, &core.saved)?;
    Ok(view_of(&dir, &core.saved))
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    app.exit(0);
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|err| err.to_string())
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|err| err.to_string())?;
    manager.is_enabled().map_err(|err| err.to_string())
}

#[tauri::command]
async fn set_word_level(app: AppHandle, state: State<'_, AppState>, level: String) -> Result<Plate, String> {
    let dir = app_dir(&app)?;
    let mut core = state.core.lock().await;
    let level = normalize_level(&level);
    let word = fetch::fetch_level_word(&level).await?;
    core.saved.level = level;
    core.saved.word = Some(word.word);
    core.saved.dictionary_url = Some(word.dictionary_url);
    core.saved.audio_url = Some(word.audio_url);
    core.saved.caption_lines = Some(word.lines);
    core.saved.error = None;
    write_saved(&dir, &core.saved)?;
    let clean = match ensure_clean(&dir, &core.saved).await {
        Ok(clean) => clean,
        Err(_) => return Ok(empty_plate(view_of(&dir, &core.saved))),
    };
    plate_from_clean(&dir, &core.saved, &clean)
}

#[tauri::command]
async fn quiz_words(state: State<'_, AppState>, count: Option<usize>) -> Result<Vec<fetch::QuizWord>, String> {
    let level = state.core.lock().await.saved.level.clone();
    // 不传就是 5 个，给纵横填字用。
    let count = count.unwrap_or(5).clamp(2, 20);
    fetch::fetch_quiz_words(&level, count).await
}

#[tauri::command]
async fn lookup_word(word: String) -> Result<fetch::DictEntry, String> {
    fetch::fetch_dict(&word).await
}

#[tauri::command]
async fn publish_jpeg(app: AppHandle, state: State<'_, AppState>, jpeg_base64: String) -> Result<ViewState, String> {
    let dir = app_dir(&app)?;
    let bytes = decode_jpeg(&jpeg_base64)?;
    let composed = dir.join("wallpaper-composed.jpg");
    fs::write(&composed, bytes).map_err(|err| err.to_string())?;
    set_mac_wallpaper(&dir, &composed)?;
    let mut core = state.core.lock().await;
    core.saved.error = None;
    write_saved(&dir, &core.saved)?;
    Ok(view_of(&dir, &core.saved))
}

#[tauri::command]
async fn set_word_box(app: AppHandle, state: State<'_, AppState>, word_box: WordBox) -> Result<Plate, String> {
    let dir = app_dir(&app)?;
    let mut core = state.core.lock().await;
    core.saved.word_box = normalize_word_box(&serde_json::to_value(&word_box).unwrap_or_default());
    let lines = ensure_caption(&mut core.saved).await;
    core.saved.caption_lines = Some(lines);
    write_saved(&dir, &core.saved)?;
    let clean = ensure_clean(&dir, &core.saved).await?;
    plate_from_clean(&dir, &core.saved, &clean)
}

#[tauri::command]
async fn preview_wallpaper(app: AppHandle, window: tauri::Window, state: State<'_, AppState>) -> Result<Option<CustomPreview>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("图片", &["jpg", "jpeg", "png", "webp", "gif", "bmp", "heic", "tif", "tiff"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|_| "这张图片没有打开".to_string())?;
    let bytes = fs::read(&path).map_err(|_| "这张图片没有打开".to_string())?;
    let image = image_ops::limit_edge(image_ops::open_image(&bytes)?);
    let (source_width, source_height) = (image.width(), image.height());
    let (target_width, target_height) = wallpaper_size(&window);
    let dir = app_dir(&app)?;
    let pending = dir.join("wallpaper-custom-pending.jpg");
    fs::write(&pending, image_ops::to_jpeg(&image, 92)?).map_err(|err| err.to_string())?;
    let preview = image_ops::preview_jpeg(&image)?;
    let name = path.file_name().and_then(|item| item.to_str()).unwrap_or("图片").to_string();
    let mut core = state.core.lock().await;
    core.pending = Some(PendingCustom {
        path: pending,
        name: name.clone(),
        target_width,
        target_height,
    });
    Ok(Some(CustomPreview {
        name,
        image_data_url: data_url(&preview),
        source_width,
        source_height,
        target_width,
        target_height,
    }))
}

#[derive(Deserialize)]
struct Focus {
    x: Option<f32>,
    y: Option<f32>,
}

#[tauri::command]
async fn confirm_wallpaper(app: AppHandle, state: State<'_, AppState>, focus: Option<Focus>) -> Result<Plate, String> {
    let dir = app_dir(&app)?;
    let mut core = state.core.lock().await;
    let pending = core.pending.take().ok_or_else(|| "请先选择一张图片".to_string())?;
    let bytes = fs::read(&pending.path).map_err(|_| "请先选择一张图片".to_string())?;
    let focus_x = focus.as_ref().and_then(|item| item.x).unwrap_or(0.5);
    let focus_y = focus.as_ref().and_then(|item| item.y).unwrap_or(0.5);
    let cropped = image_ops::crop_cover(&bytes, pending.target_width, pending.target_height, focus_x, focus_y)?;
    let dest = dir.join("wallpaper-custom.jpg");
    fs::write(&dest, &cropped).map_err(|err| err.to_string())?;
    let lines = ensure_caption(&mut core.saved).await;
    core.saved.caption_lines = Some(lines);
    core.saved.source = "custom".into();
    core.saved.image_path = Some(dest.display().to_string());
    core.saved.copyright = Some(pending.name);
    core.saved.copyright_url = None;
    core.saved.favorite_id = None;
    core.saved.photo_url = None;
    core.saved.error = None;
    write_saved(&dir, &core.saved)?;
    plate_from_clean(&dir, &core.saved, &dest)
}

#[tauri::command]
fn list_favorites(app: AppHandle) -> Result<Vec<FavoriteItem>, String> {
    let dir = app_dir(&app)?;
    let folder = favorites_dir(&dir);
    Ok(load_favorites(&dir)
        .into_iter()
        .filter(|item| folder.join(format!("{}.jpg", item.id)).exists())
        .map(|item| {
            let thumb = folder.join(format!("{}.thumb.jpg", item.id));
            FavoriteItem {
                id: item.id,
                word: item.word.unwrap_or_default(),
                copyright: item.copyright.unwrap_or_default(),
                source: if item.source == "custom" { "custom".into() } else { "bing".into() },
                thumb_url: fs::read(thumb).map(|bytes| data_url(&bytes)).unwrap_or_default(),
            }
        })
        .collect())
}

#[tauri::command]
async fn save_favorite(app: AppHandle, state: State<'_, AppState>) -> Result<ViewState, String> {
    let dir = app_dir(&app)?;
    let mut core = state.core.lock().await;
    if core.saved.image_path.as_ref().map(|path| Path::new(path).exists()).unwrap_or(false) == false {
        return Err("还没有可以收藏的壁纸".into());
    }
    let items = load_favorites(&dir);
    if current_favorite(&core.saved, &items).is_some() {
        return Ok(view_of(&dir, &core.saved));
    }
    let lines = ensure_caption(&mut core.saved).await;
    core.saved.caption_lines = Some(lines);
    let clean = ensure_clean(&dir, &core.saved).await?;
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis().to_string())
        .unwrap_or_else(|_| "favorite".into());
    let folder = favorites_dir(&dir);
    fs::create_dir_all(&folder).map_err(|err| err.to_string())?;
    let file = folder.join(format!("{id}.jpg"));
    fs::copy(&clean, &file).map_err(|err| err.to_string())?;
    if let Ok(bytes) = fs::read(&file) {
        if let Ok(thumb) = image_ops::thumb_jpeg(&bytes) {
            let _ = fs::write(folder.join(format!("{id}.thumb.jpg")), thumb);
        }
    }
    let mut items = load_favorites(&dir);
    items.insert(
        0,
        FavoriteRecord {
            id: id.clone(),
            word: core.saved.word.clone(),
            dictionary_url: core.saved.dictionary_url.clone(),
            audio_url: core.saved.audio_url.clone(),
            copyright: core.saved.copyright.clone(),
            copyright_url: core.saved.copyright_url.clone(),
            image_id: core.saved.image_id.clone(),
            caption_lines: core.saved.caption_lines.clone(),
            source: if core.saved.source == "custom" { "custom".into() } else { "bing".into() },
            word_box: core.saved.word_box.clone(),
        },
    );
    save_favorites(&dir, &items)?;
    core.saved.favorite_id = Some(id);
    write_saved(&dir, &core.saved)?;
    Ok(view_of(&dir, &core.saved))
}

#[tauri::command]
async fn remove_favorite(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<ViewState, String> {
    let dir = app_dir(&app)?;
    let items: Vec<_> = load_favorites(&dir).into_iter().filter(|item| item.id != id).collect();
    save_favorites(&dir, &items)?;
    let folder = favorites_dir(&dir);
    let _ = fs::remove_file(folder.join(format!("{id}.jpg")));
    let _ = fs::remove_file(folder.join(format!("{id}.thumb.jpg")));
    let mut core = state.core.lock().await;
    if core.saved.favorite_id.as_deref() == Some(&id) {
        core.saved.favorite_id = None;
        write_saved(&dir, &core.saved)?;
    }
    Ok(view_of(&dir, &core.saved))
}

#[tauri::command]
async fn remove_current_favorite(app: AppHandle, state: State<'_, AppState>) -> Result<ViewState, String> {
    let dir = app_dir(&app)?;
    let id = {
        let core = state.core.lock().await;
        current_favorite(&core.saved, &load_favorites(&dir)).map(|item| item.id.clone())
    };
    if let Some(id) = id {
        return remove_favorite(app, state, id).await;
    }
    let core = state.core.lock().await;
    Ok(view_of(&dir, &core.saved))
}

#[tauri::command]
async fn use_favorite(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<Plate, String> {
    let dir = app_dir(&app)?;
    let entry = load_favorites(&dir)
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "这张收藏已经不在了".to_string())?;
    let file = favorites_dir(&dir).join(format!("{}.jpg", entry.id));
    if !file.exists() {
        return Err("这张收藏已经不在了".into());
    }
    let mut core = state.core.lock().await;
    if entry.source == "custom" {
        let dest = dir.join("wallpaper-custom.jpg");
        fs::copy(&file, &dest).map_err(|err| err.to_string())?;
        core.saved.source = "custom".into();
        core.saved.image_path = Some(dest.display().to_string());
    } else {
        let _ = fs::remove_file(dir.join("wallpaper-clean.txt"));
        core.saved.source = "bing".into();
        core.saved.image_path = Some(file.display().to_string());
    }
    core.saved.image_id = entry.image_id.clone();
    core.saved.favorite_id = Some(entry.id.clone());
    core.saved.word = entry.word.clone();
    core.saved.dictionary_url = entry.dictionary_url.clone();
    core.saved.audio_url = entry.audio_url.clone();
    core.saved.copyright = entry.copyright.clone();
    core.saved.copyright_url = entry.copyright_url.clone();
    core.saved.caption_lines = entry.caption_lines.clone().or_else(|| entry.word.clone().map(|word| vec![word]));
    core.saved.word_box = entry.word_box.clone();
    core.saved.photo_url = None;
    core.saved.error = None;
    write_saved(&dir, &core.saved)?;
    let clean = ensure_clean(&dir, &core.saved).await?;
    plate_from_clean(&dir, &core.saved, &clean)
}

fn clip_frame(window: &tauri::Window) -> Option<(tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>)> {
    let outer = window.outer_size().ok()?;
    let scale = window.scale_factor().unwrap_or(1.0);
    // 子 webview 从窗口外框顶上算起。macOS 上 outer/inner 位置差是 0，标题栏仍占大约 28pt。
    let chrome = match (window.outer_position(), window.inner_position()) {
        (Ok(outer_pos), Ok(inner_pos)) => {
            let delta = inner_pos.y.saturating_sub(outer_pos.y).max(0) as u32;
            if delta == 0 {
                (28.0 * scale).round() as u32
            } else {
                delta
            }
        }
        _ => (28.0 * scale).round() as u32,
    };
    let bar = (CLIP_BAR_PX * scale).round() as u32;
    let y = chrome + bar;
    let height = outer.height.saturating_sub(y).max(1);
    Some((
        tauri::PhysicalPosition::new(0, y as i32),
        tauri::PhysicalSize::new(outer.width.max(1), height),
    ))
}

fn place_clip(window: &tauri::Window) {
    let Some(clip) = window.app_handle().get_webview(CLIP_LABEL) else {
        return;
    };
    let Some((position, size)) = clip_frame(window) else {
        return;
    };
    let _ = clip.set_position(position);
    let _ = clip.set_size(size);
}

// 看单词时把窗口放大到舒适的阅读尺寸。这里必须用 Logical：高分屏上 Physical(1080x760)
// 会变成 540x380 逻辑点，页面被压扁。同时按当前屏幕的逻辑可用区域做一下收敛，避免超出屏幕。
fn word_window_size(window: &tauri::Window) -> tauri::Size {
    const WANT_WIDTH: f64 = 1080.0;
    const WANT_HEIGHT: f64 = 760.0;
    let (max_width, max_height) = match window.current_monitor().ok().flatten() {
        Some(monitor) => {
            let scale = monitor.scale_factor().max(1.0);
            let area = monitor.work_area();
            (area.size.width as f64 / scale, area.size.height as f64 / scale)
        }
        None => (WANT_WIDTH, WANT_HEIGHT),
    };
    tauri::Size::Logical(tauri::LogicalSize::new(
        WANT_WIDTH.min(max_width).max(360.0),
        WANT_HEIGHT.min(max_height).max(280.0),
    ))
}

fn restore_home(window: &tauri::Window, home: HomeBounds) {
    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(home.x, home.y)));
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(home.width, home.height)));
}

#[tauri::command]
async fn watch_word(app: AppHandle, window: tauri::Window, state: State<'_, AppState>) -> Result<(), String> {
    let word = {
        let mut core = state.core.lock().await;
        let word = core.saved.word.clone().ok_or_else(|| "还没有单词".to_string())?;
        if core.home.is_none() {
            if let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) {
                core.home = Some(HomeBounds {
                    x: position.x,
                    y: position.y,
                    width: size.width,
                    height: size.height,
                });
            }
        }
        word
    };
    // PlayPhrase 带 X-Frame-Options: SAMEORIGIN，放进 iframe 会被留白。
    // 直接打开成窗口里的页面，这个限制不会生效。先把地址校验好再动窗口，免得失败后回不来。
    let url: tauri::Url = format!("https://www.playphrase.me/reels/en/zh-cn/{}/", urlencoding_min(&word))
        .parse()
        .map_err(|err| format!("地址无效（{err}）"))?;
    let before = window.outer_size().ok();
    let _ = window.set_size(word_window_size(&window));
    let _ = window.set_title(&format!("看单词 {word}"));
    // 窗口尺寸是异步生效的。子 webview 创建后改大小不会跟着变，所以等它变完再贴。
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        if window.outer_size().ok() != before {
            break;
        }
    }
    let opened = match app.get_webview(CLIP_LABEL) {
        Some(existing) => existing.navigate(url).map_err(|err| err.to_string()),
        None => match clip_frame(&window) {
            Some((position, size)) => window
                .add_child(WebviewBuilder::new(CLIP_LABEL, WebviewUrl::External(url)), position, size)
                .map(|_| ())
                .map_err(|err| err.to_string()),
            None => Err("窗口尺寸还没准备好".to_string()),
        },
    };
    if let Err(err) = opened {
        // 打不开就把窗口恢复回首页大小、清掉 home 记录，避免首页被压在下面又点不动。
        let mut core = state.core.lock().await;
        if let Some(home) = core.home.take() {
            restore_home(&window, home);
        }
        let _ = window.set_title("Wallish");
        return Err(err);
    }
    place_clip(&window);
    Ok(())
}

fn urlencoding_min(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[tauri::command]
async fn close_clip(app: AppHandle, window: tauri::Window, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(clip) = app.get_webview(CLIP_LABEL) {
        let _ = clip.close();
    }
    let mut core = state.core.lock().await;
    if let Some(home) = core.home.take() {
        restore_home(&window, home);
    }
    let _ = window.set_title("Wallish");
    Ok(())
}

fn show_main(app: &AppHandle) {
    // 注意：子 webview 存在时主窗口不再是 WebviewWindow，get_webview_window 会返回 None，
    // 所以这里用 get_window 拿普通 Window。
    if let Some(window) = app.get_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn spawn_switch(app: &AppHandle, random: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match switch_wallpaper(app.clone(), state, random).await {
            Ok(plate) => {
                let _ = app.emit("plate", plate);
            }
            Err(err) => {
                app.dialog().message(err).title("切换壁纸").blocking_show();
            }
        }
    });
}

/// 托盘「更换词条」：用当前等级抽一个新词，复用 set_word_level 后把 plate 推给前端重绘。
fn spawn_word(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let level = state.core.lock().await.saved.level.clone();
        match set_word_level(app.clone(), state, level).await {
            Ok(plate) => {
                let _ = app.emit("plate", plate);
            }
            Err(err) => {
                app.dialog().message(err).title("更换词条").blocking_show();
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            core: Mutex::new(Core {
                saved: SavedState::default(),
                pending: None,
                home: None,
            }),
        })
        .setup(|app| {
            if let Ok(dir) = app_dir(app.handle()) {
                let saved = read_saved(&dir);
                let state = app.state::<AppState>();
                if let Ok(mut core) = state.core.try_lock() {
                    core.saved = saved;
                };
            }
            // 开机自启时静默驻留托盘，不弹窗口。
            if std::env::args().any(|arg| arg == "--autostart") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            } else if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
            let show = tauri::menu::MenuItem::with_id(app, "show", "显示", true, None::<&str>)?;
            let today = tauri::menu::MenuItem::with_id(app, "today", "今日壁纸", true, None::<&str>)?;
            let past = tauri::menu::MenuItem::with_id(app, "past", "切换往日壁纸", true, None::<&str>)?;
            let word = tauri::menu::MenuItem::with_id(app, "word", "更换词条", true, None::<&str>)?;
            let quit = tauri::menu::MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show, &today, &past, &word, &quit])?;

            // 窗口菜单栏单独建一套菜单项：muda 的一个菜单项只能挂在一个菜单上，
            // 复用同一批 item 会让托盘菜单变空（Windows 上右键就没「退出」了）。
            #[cfg(not(target_os = "macos"))]
            {
                let a_show = tauri::menu::MenuItem::with_id(app, "show", "显示", true, None::<&str>)?;
                let a_today = tauri::menu::MenuItem::with_id(app, "today", "今日壁纸", true, None::<&str>)?;
                let a_past = tauri::menu::MenuItem::with_id(app, "past", "切换往日壁纸", true, None::<&str>)?;
                let a_word = tauri::menu::MenuItem::with_id(app, "word", "更换词条", true, None::<&str>)?;
                let a_quit = tauri::menu::MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
                let file_menu = tauri::menu::Submenu::with_items(
                    app,
                    "文件",
                    true,
                    &[&a_show, &a_today, &a_past, &a_word, &a_quit],
                )?;
                let app_menu = tauri::menu::Menu::with_items(app, &[&file_menu])?;
                app.set_menu(app_menu)?;
            }
            let mut tray = tauri::tray::TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Wallish")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "today" => spawn_switch(app, false),
                    "past" => spawn_switch(app, true),
                    "word" => spawn_word(app),
                    "quit" => {
                        QUITTING.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        show_main(tray.app_handle());
                    }
                });
            if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")) {
                tray = tray.icon(icon);
            }
            let _ = tray.build(app);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            tauri::WindowEvent::Resized(_) if window.label() == "main" => {
                place_clip(window);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            switch_wallpaper,
            publish_jpeg,
            set_word_box,
            set_word_level,
            set_wallpaper_source,
            get_autostart,
            set_autostart,
            quit_app,
            quiz_words,
            lookup_word,
            preview_wallpaper,
            confirm_wallpaper,
            list_favorites,
            save_favorite,
            remove_favorite,
            remove_current_favorite,
            use_favorite,
            watch_word,
            close_clip
        ])
        .run(tauri::generate_context!())
        .expect("启动 Wallish 失败");
}
