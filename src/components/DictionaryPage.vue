<script setup>
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { onMounted, ref, watch } from "vue";
import { errorText } from "../api.js";

const props = defineProps({
  word: { type: String, default: "" },
});
const emit = defineEmits(["back"]);

const HISTORY_KEY = "wallish.dict.history";
const query = ref(props.word || "");
const entry = ref(null);
const busy = ref(false);
const error = ref("");
const history = ref(readHistory());

function readHistory() {
  try {
    const raw = window.localStorage.getItem(HISTORY_KEY);
    const list = raw ? JSON.parse(raw) : [];
    return Array.isArray(list) ? list.filter((item) => typeof item === "string").slice(0, 8) : [];
  } catch {
    return [];
  }
}

function remember(word) {
  const next = [word, ...history.value.filter((item) => item !== word)].slice(0, 8);
  history.value = next;
  try {
    window.localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
  } catch {
    /* 存不了就只是没有历史 */
  }
}

async function search(word) {
  const target = String(word || query.value).trim();
  if (!target) {
    error.value = "先输入一个单词";
    return;
  }
  query.value = target;
  busy.value = true;
  error.value = "";
  try {
    entry.value = await invoke("lookup_word", { word: target });
    remember(entry.value.word || target);
  } catch (err) {
    entry.value = null;
    error.value = errorText(err, "没有查到这个单词");
  } finally {
    busy.value = false;
  }
}

function pick(word) {
  query.value = word;
  search(word);
}

function play(url) {
  if (url) new Audio(url).play().catch(() => {});
}

function openWeb() {
  if (entry.value && entry.value.webUrl) {
    openUrl(entry.value.webUrl).catch(() => {
      error.value = "打不开网页词典";
    });
  }
}

watch(
  () => props.word,
  (value) => {
    if (value && value !== (entry.value && entry.value.word)) search(value);
  },
);

onMounted(() => {
  if (props.word) search(props.word);
});
</script>

<template>
  <main class="wallish-dict">
    <header class="wallish-dict__bar">
      <button class="wallish-dict__back" type="button" @click="emit('back')">‹ 返回</button>
      <h1 class="wallish-dict__title">查单词</h1>
    </header>

    <form class="wallish-dict__search" @submit.prevent="search()">
      <input
        v-model="query"
        class="wallish-dict__field"
        type="text"
        placeholder="输入英文单词"
        autocomplete="off"
        spellcheck="false"
      />
      <button class="wallish-dict__go" type="submit" :disabled="busy">查询</button>
    </form>

    <div v-if="history.length && !entry" class="wallish-dict__history">
      <button v-for="item in history" :key="item" class="wallish-dict__chip" type="button" @click="pick(item)">
        {{ item }}
      </button>
    </div>

    <p v-if="error" class="wallish-home__error">{{ error }}</p>
    <p v-else-if="busy" class="wallish-dict__hint">正在查…</p>

    <article v-if="entry" class="wallish-dict__card">
      <span class="wallish-dict__tape wallish-dict__tape--left" aria-hidden="true" />
      <span class="wallish-dict__tape wallish-dict__tape--right" aria-hidden="true" />
      <div class="wallish-dict__head">
        <h2 class="wallish-dict__word">{{ entry.word }}</h2>
        <div class="wallish-dict__sounds">
          <button class="wallish-dict__sound" type="button" title="英音" @click="play(entry.ukAudio)">英</button>
          <button class="wallish-dict__sound" type="button" title="美音" @click="play(entry.usAudio)">美</button>
        </div>
      </div>
      <p v-if="entry.phonetic" class="wallish-dict__phone">/{{ entry.phonetic }}/</p>

      <div v-if="entry.forms && entry.forms.length" class="wallish-dict__forms">
        <span v-for="form in entry.forms" :key="form.value" class="wallish-dict__form">
          <em>{{ form.name }}</em>
          <strong>{{ form.value }}</strong>
        </span>
      </div>

      <ul v-if="entry.senses && entry.senses.length" class="wallish-dict__senses">
        <li v-for="(sense, index) in entry.senses" :key="`${sense.pos}-${index}`" class="wallish-dict__sense">
          <span v-if="sense.pos" class="wallish-dict__pos">{{ sense.pos }}</span>
          <span class="wallish-dict__text">{{ sense.text }}</span>
        </li>
      </ul>

      <div v-if="entry.examples && entry.examples.length" class="wallish-dict__examples">
        <p class="wallish-dict__label">例句</p>
        <div v-for="(example, index) in entry.examples" :key="index" class="wallish-dict__example">
          <p class="wallish-dict__en">{{ example.en }}</p>
          <p v-if="example.zh" class="wallish-dict__zh">{{ example.zh }}</p>
        </div>
      </div>
    </article>

    <p v-if="!entry && !busy && !error" class="wallish-dict__hint">输入想查的单词，或者点下面的历史。</p>

    <div v-if="entry" class="wallish-dict__foot">
      <button class="wallish-dict__link" type="button" @click="openWeb">在浏览器里打开</button>
    </div>
  </main>
</template>