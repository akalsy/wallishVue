<script setup>
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { errorText } from "../api.js";

const props = defineProps({
  level: { type: String, default: "" },
  embedded: { type: Boolean, default: false },
});
defineOptions({ name: "WordGame" });
const emit = defineEmits(["back"]);

const LEVEL_LABELS = {
  cet4: "四级",
  cet6: "六级",
  kaoyan: "考研",
  ielts: "雅思",
  toefl: "托福",
  gre: "GRE",
};

const rows = ref([]);
const loading = ref(false);
const error = ref("");
const dealSeq = ref(0);

const levelLabel = computed(() => LEVEL_LABELS[props.level] || props.level);
const solvedCount = computed(() => rows.value.filter((row) => row.solved).length);
const allSolved = computed(() => rows.value.length > 0 && solvedCount.value === rows.value.length);

function randomIndexes(length, hide) {
  const indexes = Array.from({ length }, (_, index) => index);
  for (let i = indexes.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    [indexes[i], indexes[j]] = [indexes[j], indexes[i]];
  }
  return indexes.slice(0, hide);
}

function buildRow(item) {
  const letters = item.word.split("");
  // 每词挖 40% 左右，至少 2 个、至多留 1 个提示字母。
  const hide = Math.min(letters.length - 1, Math.max(2, Math.round(letters.length * 0.4)));
  const hidden = new Set(randomIndexes(letters.length, hide));
  return {
    ...item,
    cells: letters.map((letter, index) => ({
      letter,
      fixed: !hidden.has(index),
      text: "",
      hint: false,
      right: true,
    })),
    solved: false,
    givenUp: false,
  };
}

async function deal() {
  loading.value = true;
  error.value = "";
  try {
    rows.value = (await invoke("quiz_words")).map(buildRow);
    dealSeq.value += 1;
    openFirstBlank(0);
  } catch (err) {
    rows.value = [];
    error.value = errorText(err, "没有拿到单词");
  } finally {
    loading.value = false;
  }
}

function gid(rowIndex, cellIndex) {
  return `wallish-game-${rowIndex}-${cellIndex}`;
}

function focusCell(rowIndex, cellIndex, select) {
  const input = document.getElementById(gid(rowIndex, cellIndex));
  if (!input) return;
  input.focus();
  if (select) input.select();
}

function openFirstBlank(rowIndex) {
  const row = rows.value[rowIndex];
  if (!row) return;
  for (let i = 0; i < row.cells.length; i += 1) {
    if (!row.cells[i].fixed && !row.cells[i].text) {
      focusCell(rowIndex, i, true);
      return;
    }
  }
}

function nextBlank(row, from) {
  for (let i = from + 1; i < row.cells.length; i += 1) {
    if (!row.cells[i].fixed && !row.cells[i].text) return i;
  }
  for (let i = 0; i <= from; i += 1) {
    if (!row.cells[i].fixed && !row.cells[i].text) return i;
  }
  return -1;
}

function prevBlank(row, from) {
  for (let i = from - 1; i >= 0; i -= 1) {
    if (!row.cells[i].fixed) return i;
  }
  return -1;
}

function allFilled(row) {
  return row.cells.every((cell) => cell.fixed || cell.text);
}

function checkRow(row) {
  let correct = true;
  for (const cell of row.cells) {
    if (cell.fixed) continue;
    const ok = cell.text.toLowerCase() === cell.letter;
    cell.right = ok;
    correct = correct && ok;
  }
  row.solved = correct;
}

function onInput(row, rowIndex, cellIndex, event) {
  if (row.solved || row.givenUp) return;
  const cell = row.cells[cellIndex];
  const typed = Array.from(event.target.value || "").reverse().find((ch) => /[a-zA-Z]/.test(ch)) || "";
  const text = typed.toUpperCase();
  cell.text = text;
  cell.right = true;
  event.target.value = text;
  if (allFilled(row)) {
    checkRow(row);
    if (row.solved) return;
  }
  const next = nextBlank(row, cellIndex);
  if (next >= 0) focusCell(rowIndex, next, true);
}

function onKeydown(row, rowIndex, cellIndex, event) {
  if (row.solved || row.givenUp) return;
  if (event.key === "Backspace" && !row.cells[cellIndex].text) {
    const prev = prevBlank(row, cellIndex);
    if (prev >= 0) {
      event.preventDefault();
      row.cells[prev].text = "";
      row.cells[prev].right = true;
      focusCell(rowIndex, prev, true);
    }
  } else if (event.key === "ArrowLeft") {
    const prev = prevBlank(row, cellIndex);
    if (prev >= 0) {
      event.preventDefault();
      focusCell(rowIndex, prev, true);
    }
  } else if (event.key === "ArrowRight" || event.key === "Enter") {
    const next = nextBlank(row, cellIndex);
    if (next >= 0) {
      event.preventDefault();
      focusCell(rowIndex, next, true);
    }
  }
}

function hint(row, rowIndex) {
  if (row.solved || row.givenUp) return;
  const target = row.cells.findIndex((cell) => !cell.fixed && (!cell.text || !cell.right));
  if (target < 0) return;
  const cell = row.cells[target];
  cell.text = cell.letter.toUpperCase();
  cell.hint = true;
  cell.right = true;
  if (allFilled(row)) {
    checkRow(row);
    if (row.solved) return;
  }
  focusCell(rowIndex, target, true);
}

function reveal(row) {
  if (row.solved || row.givenUp) return;
  for (const cell of row.cells) {
    if (cell.fixed) continue;
    cell.text = cell.letter.toUpperCase();
    cell.right = true;
  }
  row.givenUp = true;
}

function play(row) {
  if (!row.audioUrl) return;
  try {
    new Audio(row.audioUrl).play().catch(() => {});
  } catch (err) {
    // 发音放不出来就算了，不影响答题。
  }
}

function lookup(row) {
  if (!row.dictionaryUrl) return;
  openUrl(row.dictionaryUrl).catch(() => {});
}

onMounted(deal);
</script>

<template>
  <main class="wallish-game" :class="{ 'wallish-game--embedded': props.embedded }">
    <header v-if="!props.embedded" class="wallish-game__bar">
      <button class="wallish-game__back" type="button" @click="emit('back')">‹ 返回</button>
      <h1 class="wallish-game__title">填字游戏</h1>
      <span class="wallish-game__score">{{ solvedCount }}/{{ rows.length }}</span>
    </header>
    <p class="wallish-game__intro">{{ levelLabel }}词库随机 5 个词。挖掉的格子自己填，音标和中文是提示。</p>
    <div v-if="loading" class="wallish-game__empty">正在抽单词…</div>
    <template v-else>
      <div v-if="error" class="wallish-game__fail">
        <p class="wallish-home__error">{{ error }}</p>
        <button class="wallish-game__tool" type="button" @click="deal">重试</button>
      </div>
      <template v-else>
        <p v-if="allSolved" class="wallish-game__done">5 个全对！🎉 换一批继续。</p>
        <div class="wallish-game__list">
          <article
            v-for="(row, rowIndex) in rows"
            :key="`${dealSeq}-${rowIndex}`"
            class="wallish-game__card"
            :class="{ 'wallish-game__card--done': row.solved }"
          >
            <div class="wallish-game__meta">
              <span class="wallish-game__index">No.{{ rowIndex + 1 }}</span>
              <span v-if="row.phone" class="wallish-game__phone">/{{ row.phone }}/</span>
              <button
                class="wallish-game__tool"
                type="button"
                :disabled="!row.audioUrl"
                title="读一遍"
                @click="play(row)"
              >
                🔊
              </button>
              <button
                class="wallish-game__tool"
                type="button"
                :disabled="!row.dictionaryUrl"
                title="有道词典"
                @click="lookup(row)"
              >
                词典
              </button>
            </div>
            <p v-for="(line, index) in row.lines" :key="index" class="wallish-game__clue">{{ line }}</p>
            <div class="wallish-game__cells">
              <template v-for="(cell, cellIndex) in row.cells" :key="cellIndex">
                <span v-if="cell.fixed" class="wallish-game__cell wallish-game__cell--fixed">{{ cell.letter.toUpperCase() }}</span>
                <input
                  v-else
                  :id="gid(rowIndex, cellIndex)"
                  class="wallish-game__cell"
                  :class="{
                    'wallish-game__cell--wrong': !cell.right,
                    'wallish-game__cell--hint': cell.hint,
                  }"
                  type="text"
                  maxlength="2"
                  autocomplete="off"
                  autocorrect="off"
                  autocapitalize="characters"
                  spellcheck="false"
                  :value="cell.text"
                  :readonly="row.solved || row.givenUp"
                  :aria-label="`第 ${rowIndex + 1} 个词第 ${cellIndex + 1} 个字母`"
                  @input="onInput(row, rowIndex, cellIndex, $event)"
                  @keydown="onKeydown(row, rowIndex, cellIndex, $event)"
                  @focus="$event.target.select()"
                />
              </template>
            </div>
            <div class="wallish-game__tools">
              <button class="wallish-game__tool" type="button" :disabled="row.solved || row.givenUp" @click="hint(row, rowIndex)">
                提示一个
              </button>
              <button class="wallish-game__tool" type="button" :disabled="row.solved || row.givenUp" @click="reveal(row)">
                揭晓答案
              </button>
              <span v-if="row.solved" class="wallish-game__ok">答对了 ✓</span>
              <span v-else-if="row.givenUp" class="wallish-game__give">{{ row.word.toUpperCase() }}</span>
            </div>
          </article>
        </div>
        <div class="wallish-game__actions">
          <button class="wallish-game__shuffle" type="button" :disabled="loading" @click="deal">换一批</button>
        </div>
      </template>
    </template>
  </main>
</template>
