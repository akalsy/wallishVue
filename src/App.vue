<script setup>
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { computed, onMounted, ref } from "vue";
import { applyPlate, errorText } from "./api.js";
import ClipPage from "./components/ClipPage.vue";
import CustomPreview from "./components/CustomPreview.vue";
import FavoritesPage from "./components/FavoritesPage.vue";
import SettingsPage from "./components/SettingsPage.vue";

const defaultBox = { position: "bottom", background: "#1f2a36", size: "medium", caption: true };
const snapshot = ref(null);
const clip = ref(null);
const settingsOpen = ref(false);
const customPreview = ref(null);
const favoritesOpen = ref(false);
const favorites = ref([]);
const busy = ref(false);
const saving = ref(false);
const error = ref("");

const wordBox = computed(() => (snapshot.value && snapshot.value.wordBox) || defaultBox);
const settingsKey = computed(() => {
  const box = wordBox.value;
  return `${box.position}|${box.background}|${box.size}|${box.caption === false ? "0" : "1"}`;
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

async function saveWordBox(nextBox) {
  saving.value = true;
  error.value = "";
  try {
    await acceptPlate(await invoke("set_word_box", { wordBox: nextBox }));
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
    :saving="saving"
    :error="error"
    @save="saveWordBox"
    @back="settingsOpen = false"
  />
  <main v-else class="wallish-home">
    <header class="wallish-home__header">
      <div class="wallish-home__heading">
        <p class="wallish-home__kicker">Wallish</p>
        <h1 class="wallish-home__title">{{ (snapshot && snapshot.word) || "等待壁纸" }}</h1>
      </div>
      <button class="wallish-home__settings" type="button" aria-label="设置" @click="settingsOpen = true">
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
    <div class="wallish-home__stage">
      <img v-if="snapshot && snapshot.imageDataUrl" class="wallish-home__photo" :src="snapshot.imageDataUrl" :alt="snapshot.copyright || '壁纸'" />
      <div v-else class="wallish-home__photo wallish-home__photo--empty">还没有图片</div>
    </div>
    <p class="wallish-home__copy">{{ (snapshot && snapshot.copyright) || "点下面的按钮换一张 Bing 每日壁纸。" }}</p>
    <p v-if="error" class="wallish-home__error">{{ error }}</p>
    <div class="wallish-home__actions">
      <button class="wallish-home__button wallish-home__button--today" type="button" :disabled="busy" @click="changeWallpaper(false)">今日壁纸</button>
      <button class="wallish-home__button wallish-home__button--past" type="button" :disabled="busy" @click="changeWallpaper(true)">切换往日壁纸</button>
      <button class="wallish-home__button wallish-home__button--custom" type="button" :disabled="busy" @click="chooseWallpaper">自定义壁纸</button>
    </div>
    <div class="wallish-home__links">
      <button class="wallish-home__link" type="button" :disabled="busy || !(snapshot && snapshot.imageDataUrl)" @click="toggleFavorite">
        {{ snapshot && snapshot.favorited ? "已收藏" : "收藏" }}
      </button>
      <button class="wallish-home__link" type="button" :disabled="busy" @click="openFavoriteList">收藏列表</button>
      <button v-if="snapshot && snapshot.dictionaryUrl" class="wallish-home__link" type="button" @click="openDictionary">查单词</button>
      <button v-if="snapshot && snapshot.word" class="wallish-home__link" type="button" @click="watchWord">看单词 {{ snapshot.word }}</button>
    </div>
    <p class="wallish-home__hint">关掉窗口后程序还在菜单栏。退出请用托盘菜单。</p>
  </main>
</template>
