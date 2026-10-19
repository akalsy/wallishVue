<script setup>
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { computed, onMounted, ref } from "vue";
import { applyPlate, errorText } from "./api.js";
import ClipPage from "./components/ClipPage.vue";
import CustomPreview from "./components/CustomPreview.vue";
import FavoritesPage from "./components/FavoritesPage.vue";
import GameHub from "./components/GameHub.vue";
import SettingsPage from "./components/SettingsPage.vue";

const defaultBox = { position: "bottom", background: "#1f2a36", size: "medium", caption: true };
const snapshot = ref(null);
const clip = ref(null);
const settingsOpen = ref(false);
const customPreview = ref(null);
const favoritesOpen = ref(false);
const gameOpen = ref(false);
const favorites = ref([]);
const busy = ref(false);
const saving = ref(false);
const error = ref("");

const wordBox = computed(() => (snapshot.value && snapshot.value.wordBox) || defaultBox);
const level = computed(() => (snapshot.value && snapshot.value.level) || "cet4");
const wallpaperSource = computed(() => (snapshot.value && snapshot.value.wallpaperSource) || "auto");

const LEVEL_LABELS = { cet4: "四级", cet6: "六级", kaoyan: "考研", ielts: "雅思", toefl: "托福", gre: "GRE" };
const SOURCE_LABELS = { bing: "Bing 每日壁纸", wallhaven: "Wallhaven", picsum: "Lorem Picsum" };

const levelLabel = computed(() => LEVEL_LABELS[level.value] || level.value);
const wordText = computed(() => (snapshot.value && snapshot.value.word) || "");
const captionText = computed(() => (snapshot.value && snapshot.value.captionLines) || []);
const hasCJK = (text) => /[\u4e00-\u9fa5]/u.test(text);
const phonetic = computed(() => {
  const title = captionText.value[0] || "";
  const start = title.indexOf("/");
  const end = title.lastIndexOf("/");
  return start >= 0 && end > start ? title.slice(start, end + 1) : "";
});
const gloss = computed(() => captionText.value.slice(1).find((line) => hasCJK(line)) || "");
const sentence = computed(() => captionText.value.slice(1).find((line) => line.length > 12 && !hasCJK(line)) || "");
const sourceLabel = computed(() => SOURCE_LABELS[snapshot.value && snapshot.value.wallpaperSource] || "当前壁纸");

function playWord() {
  if (snapshot.value && snapshot.value.audioUrl) {
    new Audio(snapshot.value.audioUrl).play().catch(() => {});
  }
}
const settingsKey = computed(() => {
  const box = wordBox.value;
  return `${box.position}|${box.background}|${box.size}|${box.caption === false ? "0" : "1"}|${level.value}|${wallpaperSource.value}`;
});

async function acceptPlate(plate) {
  const next = await applyPlate(plate);
  snapshot.value = next;
  error.value = next && next.error ? next.error : "";
}

onMounted(async () => {
  try {
    snapshot.value = await invoke("get_state");
  } catch (err) {
    error.value = errorText(err, "没有读到当前壁纸");
  }
  listen("plate", (event) => {
    acceptPlate(event.payload).catch((err) => {
      error.value = errorText(err, "切换失败");
      busy.value = false;
    });
  });
});

async function changeWallpaper(random) {
  busy.value = true;
  error.value = "";
  try {
    await acceptPlate(await invoke("switch_wallpaper", { random }));
  } catch (err) {
    error.value = errorText(err, "切换失败");
  } finally {
    busy.value = false;
  }
}

async function chooseWallpaper() {
  busy.value = true;
  error.value = "";
  try {
    const preview = await invoke("preview_wallpaper");
    if (preview) {
      clip.value = null;
      settingsOpen.value = false;
      favoritesOpen.value = false;
      customPreview.value = preview;
    }
  } catch (err) {
    error.value = errorText(err, "这张图片没有打开");
  } finally {
    busy.value = false;
  }
}

async function confirmWallpaper(focus) {
  busy.value = true;
  error.value = "";
  try {
    await acceptPlate(await invoke("confirm_wallpaper", { focus }));
    customPreview.value = null;
  } catch (err) {
    error.value = errorText(err, "没有换成这张图片");
  } finally {
    busy.value = false;
  }
}

async function openFavoriteList() {
  error.value = "";
  favoritesOpen.value = true;
  try {
    favorites.value = await invoke("list_favorites");
  } catch (err) {
    error.value = errorText(err, "收藏列表没有打开");
  }
}

async function toggleFavorite() {
  busy.value = true;
  error.value = "";
  try {
    snapshot.value = snapshot.value && snapshot.value.favorited
      ? await invoke("remove_current_favorite")
      : await invoke("save_favorite");
  } catch (err) {
    error.value = errorText(err, "收藏没有保存");
  } finally {
    busy.value = false;
  }
}

async function useFavorite(id) {
  busy.value = true;
  error.value = "";
  try {
    await acceptPlate(await invoke("use_favorite", { id }));
    favoritesOpen.value = false;
  } catch (err) {
    error.value = errorText(err, "没有换成这张收藏");
  } finally {
    busy.value = false;
  }
}

async function removeFavorite(id) {
  busy.value = true;
  error.value = "";
  try {
    snapshot.value = await invoke("remove_favorite", { id });
    favorites.value = await invoke("list_favorites");
  } catch (err) {
    error.value = errorText(err, "没有取消收藏");
  } finally {
    busy.value = false;
  }
}

async function saveWordBox(draft) {
  saving.value = true;
  error.value = "";
  try {
    const { level: nextLevel, wallpaperSource: nextSource, ...wordBox } = draft;
    await acceptPlate(await invoke("set_word_box", { wordBox }));
    if (nextLevel && nextLevel !== level.value) {
      await acceptPlate(await invoke("set_word_level", { level: nextLevel }));
    }
    if (nextSource && nextSource !== wallpaperSource.value) {
      snapshot.value = await invoke("set_wallpaper_source", { source: nextSource });
    }
  } catch (err) {
    error.value = errorText(err, "设置没有保存");
  } finally {
    saving.value = false;
  }
}

async function watchWord() {
  try {
    await invoke("watch_word");
    clip.value = { word: snapshot.value && snapshot.value.word };
    settingsOpen.value = false;
  } catch (err) {
    error.value = errorText(err, "打不开看单词");
  }
}

async function backHome() {
  clip.value = null;
  try {
    await invoke("close_clip");
  } catch (err) {
    error.value = errorText(err, "返回失败");
  }
}

function openDictionary() {
  if (snapshot.value && snapshot.value.dictionaryUrl) {
    openUrl(snapshot.value.dictionaryUrl).catch((err) => {
      error.value = errorText(err, "打不开词典");
    });
  }
}
</script>

<template>
  <ClipPage v-if="clip" :word="clip.word" @back="backHome" />
  <GameHub v-else-if="gameOpen" :level="level" @back="gameOpen = false" />
  <FavoritesPage
    v-else-if="favoritesOpen"
    :items="favorites"
    :busy="busy"
    :error="error"
    @use="useFavorite"
    @remove="removeFavorite"
    @back="favoritesOpen = false"
  />
  <CustomPreview
    v-else-if="customPreview"
    :preview="customPreview"
    :busy="busy"
    :error="error"
    @confirm="confirmWallpaper"
    @pick="chooseWallpaper"
    @back="customPreview = null"
  />
  <SettingsPage
    v-else-if="settingsOpen"
    :key="settingsKey"
    :image-url="snapshot && snapshot.imageDataUrl"
    :box="wordBox"
    :level="level"
    :wallpaper-source="wallpaperSource"
    :saving="saving"
    :error="error"
    @save="saveWordBox"
    @back="settingsOpen = false"
  />
  <main v-else class="wallish-home">
    <header class="wallish-home__header">
      <p class="wallish-home__brand">WALLISH</p>
      <button class="wallish-home__settings" type="button" title="设置" aria-label="设置" @click="settingsOpen = true">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path fill="none" stroke="currentColor" stroke-width="1.6" d="M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4Z" />
          <path
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linejoin="round"
            d="M19.4 13.5a7.7 7.7 0 0 0 .1-1.5 7.7 7.7 0 0 0-.1-1.5l2-1.6-2-3.4-2.4 1a7.4 7.4 0 0 0-2.6-1.5L14 2h-4l-.4 2.5a7.4 7.4 0 0 0-2.6 1.5l-2.4-1-2 3.4 2 1.6a7.7 7.7 0 0 0-.1 1.5 7.7 7.7 0 0 0 .1 1.5l-2 1.6 2 3.4 2.4-1a7.4 7.4 0 0 0 2.6 1.5L10 22h4l.4-2.5a7.4 7.4 0 0 0 2.6-1.5l2.4 1 2-3.4-2-1.6Z"
          />
        </svg>
      </button>
    </header>

    <section class="wallish-home__wordbar">
      <div class="wallish-home__wordmain">
        <div class="wallish-home__wordline">
          <h1 class="wallish-home__word">{{ wordText || "等待壁纸" }}</h1>
          <button v-if="snapshot && snapshot.audioUrl" class="wallish-home__sound" type="button" title="朗读发音" aria-label="朗读发音" @click="playWord">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M11 5 6 9H2v6h4l5 4V5z" />
              <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
              <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
            </svg>
          </button>
        </div>
        <p class="wallish-home__wordsub">
          <span v-if="phonetic" class="wallish-home__phone">{{ phonetic }}</span>
          <span v-if="gloss" class="wallish-home__gloss">{{ gloss }}</span>
        </p>
      </div>
      <span class="wallish-home__tag">词库 · {{ levelLabel }}</span>
    </section>

    <div class="wallish-home__frame">
      <span class="wallish-home__tape wallish-home__tape--left" aria-hidden="true" />
      <span class="wallish-home__tape wallish-home__tape--right" aria-hidden="true" />
      <div class="wallish-home__photo-box">
        <img v-if="snapshot && snapshot.imageDataUrl" class="wallish-home__photo" :src="snapshot.imageDataUrl" :alt="snapshot.copyright || '壁纸'" />
        <div v-else class="wallish-home__photo wallish-home__photo--empty">还没有图片</div>
        <div v-if="wordBox.caption === false && (wordText || phonetic || gloss || sentence)" class="wallish-home__card">
          <div class="wallish-home__cardline">
            <span class="wallish-home__cardword">{{ wordText }}</span>
            <span v-if="phonetic" class="wallish-home__cardphone">{{ phonetic }}</span>
          </div>
          <p v-if="gloss" class="wallish-home__cardgloss">{{ gloss }}</p>
          <p v-if="sentence" class="wallish-home__cardsentence">{{ sentence }}</p>
        </div>
      </div>
      <div class="wallish-home__credit">
        <span class="wallish-home__creditsource">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z" />
            <circle cx="12" cy="13" r="3" />
          </svg>
          {{ sourceLabel }}
        </span>
        <span class="wallish-home__creditauthor">{{ (snapshot && snapshot.copyright) || "" }}</span>
      </div>
    </div>

    <p v-if="error" class="wallish-home__error">{{ error }}</p>

    <div class="wallish-home__actions">
      <button class="wallish-home__button wallish-home__button--today" type="button" :disabled="busy" @click="changeWallpaper(false)">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
        </svg>
        <span>今日壁纸</span>
      </button>
      <button class="wallish-home__button wallish-home__button--past" type="button" :disabled="busy" @click="changeWallpaper(true)">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M2 18h1.4c1.3 0 2.5-.6 3.3-1.7l6.1-8.6c.8-1.1 2-1.7 3.3-1.7H22" />
          <path d="m18 2 4 4-4 4" />
          <path d="M2 6h1.9c1.5 0 2.9.9 3.6 2.2" />
          <path d="M22 18h-5.9c-1.3 0-2.6-.7-3.3-1.8l-.5-.8" />
          <path d="m18 14 4 4-4 4" />
        </svg>
        <span>切换往日</span>
      </button>
      <button class="wallish-home__button wallish-home__button--custom" type="button" :disabled="busy" @click="chooseWallpaper">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect width="18" height="18" x="3" y="3" rx="2" />
          <circle cx="9" cy="9" r="2" />
          <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" />
        </svg>
        <span>自定义壁纸</span>
      </button>
    </div>

    <nav class="wallish-home__desk">
      <button class="wallish-home__opt" type="button" :disabled="busy || !(snapshot && snapshot.imageDataUrl)" @click="toggleFavorite">
        <span class="wallish-home__opticon wallish-home__opticon--fav">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z" :fill="snapshot && snapshot.favorited ? '#ef4444' : 'none'" :stroke="snapshot && snapshot.favorited ? '#ef4444' : 'currentColor'" />
          </svg>
        </span>
        <span class="wallish-home__optlabel">{{ snapshot && snapshot.favorited ? "已收藏" : "收藏" }}</span>
      </button>
      <button class="wallish-home__opt" type="button" :disabled="busy" @click="openFavoriteList">
        <span class="wallish-home__opticon wallish-home__opticon--list">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z" />
          </svg>
        </span>
        <span class="wallish-home__optlabel">收藏列表</span>
      </button>
      <button v-if="snapshot && snapshot.dictionaryUrl" class="wallish-home__opt" type="button" @click="openDictionary">
        <span class="wallish-home__opticon wallish-home__opticon--search">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.3-4.3" />
          </svg>
        </span>
        <span class="wallish-home__optlabel">查单词</span>
      </button>
      <button class="wallish-home__opt" type="button" @click="gameOpen = true">
        <span class="wallish-home__opticon wallish-home__opticon--game">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <line x1="6" x2="10" y1="12" y2="12" />
            <line x1="8" x2="8" y1="9" y2="15" />
            <line x1="15" x2="15.01" y1="12" y2="12" />
            <line x1="18" x2="18.01" y1="10" y2="10" />
            <path d="M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258-.007-.05-.011-.1-.017-.151A4 4 0 0 0 17.32 5z" />
          </svg>
        </span>
        <span class="wallish-home__optlabel">填字游戏</span>
      </button>
      <button v-if="snapshot && snapshot.word" class="wallish-home__opt" type="button" @click="watchWord">
        <span class="wallish-home__opticon wallish-home__opticon--word">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
            <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
          </svg>
        </span>
        <span class="wallish-home__optlabel">看单词</span>
      </button>
    </nav>
    <p class="wallish-home__hint">关掉窗口后程序还在菜单栏。退出请用托盘菜单。</p>
  </main>
</template>
