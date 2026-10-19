<script setup>
import { computed, nextTick, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "../api.js";

const props = defineProps({
  level: { type: String, default: "" },
  embedded: { type: Boolean, default: false },
});
defineOptions({ name: "WordCross" });
const emit = defineEmits(["back"]);

const LEVEL_LABELS = {
  cet4: "四级",
  cet6: "六级",
  kaoyan: "考研",
  ielts: "雅思",
  toefl: "托福",
  gre: "GRE",
};

const board = ref(null);
const loading = ref(false);
const error = ref("");
const dealSeq = ref(0);
const stats = reactive({ right: 0, wrong: 0 });
const selected = ref({ k: "", dir: "across" });

const levelLabel = computed(() => LEVEL_LABELS[props.level] || props.level);
const won = computed(() => !!board.value && board.value.words.length > 0 && board.value.words.every((word) => word.solved));
const blankCount = computed(() => {
  if (!board.value) return 0;
  return Object.values(board.value.cells).filter((cell) => !cell.fixed).length;
});
const score = computed(() => Math.max(0, Math.min(100, stats.right * 20 - stats.wrong * 5)));

const K = (x, y) => `${x},${y}`;
const parseKey = (key) => key.split(",").map(Number);

// ── 拼盘：把 5 个词摆成互相交叉的格子 ──────────────────────────────

function buildBoard(items) {
  const sorted = items.slice().filter((item) => item.word).sort((a, b) => b.word.length - a.word.length);
  if (!sorted.length) {
    throw new Error("没有拿到单词");
  }
  const letters = new Map();
  const owners = new Map();
  const placed = [];

  function commit(dir, x, y, item) {
    const lettersArr = item.word.split("");
    const keys = [];
    for (let i = 0; i < lettersArr.length; i += 1) {
      const key = dir === "across" ? K(x + i, y) : K(x, y + i);
      letters.set(key, lettersArr[i]);
      const own = owners.get(key) || { across: false, down: false };
      own[dir] = true;
      owners.set(key, own);
      keys.push(key);
    }
    placed.push({ dir, x, y, item, keys });
  }

  function boundaryOf(x, y, word, dir) {
    // 词前、词后那一格必须为空，不能和别的词粘成长条。
    const before = dir === "across" ? K(x - 1, y) : K(x, y - 1);
    const after = dir === "across" ? K(x + word.length, y) : K(x, y + word.length);
    return { before, after };
  }

  function perpendicularOf(dir, x, y, i) {
    // 非交叉格的两旁（垂直方向）必须空，避免两条并排的词缝里冒字母。
    return dir === "across" ? [K(x + i, y - 1), K(x + i, y + 1)] : [K(x - 1, y), K(x + 1, y)];
  }

  // 不接交叉点的整条空位。
  function freeSlot(dir, x, y, word) {
    const { before, after } = boundaryOf(x, y, word, dir);
    if (letters.has(before) || letters.has(after)) return false;
    for (let i = 0; i < word.length; i += 1) {
      const key = dir === "across" ? K(x + i, y) : K(x, y + i);
      if (letters.has(key)) return false;
      const [p1, p2] = perpendicularOf(dir, x, y, i);
      if (letters.has(p1) || letters.has(p2)) return false;
    }
    return true;
  }

  // 以已有字母当交叉点的情况。词头词尾照样查；只有交叉格允许踩旧字母。
  function fitsCross(dir, x, y, word, crossIndex) {
    const { before, after } = boundaryOf(x, y, word, dir);
    if (letters.has(before) || letters.has(after)) return false;
    for (let i = 0; i < word.length; i += 1) {
      if (i === crossIndex) continue;
      const key = dir === "across" ? K(x + i, y) : K(x, y + i);
      if (letters.has(key)) return false;
      const [p1, p2] = perpendicularOf(dir, x, y, i);
      if (letters.has(p1) || letters.has(p2)) return false;
    }
    return true;
  }

  function tryCross(item) {
    const word = item.word.split("");
    const options = [];
    for (const key of Array.from(letters.keys())) {
      const [cx, cy] = parseKey(key);
      const own = owners.get(key);
      // 一个格子已经是纵横十字点就不再加词；单向词只允许从垂直方向穿过。
      const dirs = [];
      if (own.down && !own.across) dirs.push("across");
      if (own.across && !own.down) dirs.push("down");
      const cross = letters.get(key);
      for (const dir of dirs) {
        for (let i = 0; i < word.length; i += 1) {
          if (word[i] !== cross) continue;
          const x = dir === "across" ? cx - i : cx;
          const y = dir === "across" ? cy : cy - i;
          if (fitsCross(dir, x, y, word, i)) options.push({ dir, x, y });
        }
      }
    }
    if (!options.length) return false;
    const pick = options[Math.floor(Math.random() * options.length)];
    commit(pick.dir, pick.x, pick.y, item);
    return true;
  }

  function boundsNow() {
    let minX = 0;
    let minY = 0;
    let maxX = 0;
    let maxY = 0;
    for (const key of letters.keys()) {
      const [x, y] = parseKey(key);
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
    return { minX, minY, maxX, maxY };
  }

  commit("across", 0, 0, sorted[0]);

  let pending = sorted.slice(1);
  for (let round = 0; round < 3 && pending.length; round += 1) {
    const rest = [];
    for (const item of pending) {
      if (!tryCross(item)) rest.push(item);
    }
    if (rest.length === pending.length) {
      pending = rest;
      break;
    }
    pending = rest;
  }

  // 没接上的词，到盘面外找一条空行单独放。
  let cursorY = boundsNow().maxY + 2;
  for (const item of pending) {
    let parked = false;
    for (let y = cursorY; y <= cursorY + 6 && !parked; y += 1) {
      for (let x = -2; x <= 2 && !parked; x += 1) {
        if (freeSlot("across", x, y, item.word.split(""))) {
          commit("across", x, y, item);
          cursorY = y + 2;
          parked = true;
        }
      }
    }
    if (!parked) {
      // 兜底：离得更远直接放（盘子边缘不会向下延伸到这里）。
      commit("across", 0, cursorY + 8, item);
      cursorY += 10;
    }
  }

  // ── 摆完之后摊平成渲染用的棋盘 ──
  const b = boundsNow();
  const offsetX = -b.minX;
  const offsetY = -b.minY;
  const cols = b.maxX - b.minX + 1;
  const rows = b.maxY - b.minY + 1;

  const cells = {};
  const words = placed.map((p) => ({
    dir: p.dir,
    sx: p.x + offsetX,
    sy: p.y + offsetY,
    keys: p.keys.map((key) => {
      const [x, y] = parseKey(key);
      return K(x + offsetX, y + offsetY);
    }),
    item: p.item,
    number: 0,
    solved: false,
    givenUp: false,
  }));

  for (let y = 0; y < rows; y += 1) {
    for (let x = 0; x < cols; x += 1) {
      const original = K(x - offsetX, y - offsetY);
      if (!letters.has(original)) continue;
      cells[K(x, y)] = { letter: letters.get(original), text: "", wrong: false, ok: false, hint: false, fixed: false, number: 0 };
    }
  }

  // 预填几个提示字母：优先交叉格，越靠盘面中间越好；每条词至少留 1 个空。
  const ownerCount = new Map();
  const blanksLeft = new Map();
  for (const word of words) {
    blanksLeft.set(word, word.keys.length);
    for (const key of word.keys) {
      ownerCount.set(key, (ownerCount.get(key) || 0) + 1);
    }
  }
  const cellKeys = Object.keys(cells);
  const distanceFromCenter = (key) => {
    const [x, y] = parseKey(key);
    return Math.hypot(x - cols / 2, y - rows / 2);
  };
  const revealSeeds = (pool, quota) => {
    if (quota <= 0 || !pool.length) return 0;
    // 先按离中心的距离排，再在前一半的靠中区间里随机抽。
    const window = pool.slice(0, Math.min(pool.length, quota * 2));
    for (let i = window.length - 1; i > 0; i -= 1) {
      const j = Math.floor(Math.random() * (i + 1));
      [window[i], window[j]] = [window[j], window[i]];
    }
    let revealed = 0;
    for (const key of window) {
      if (revealed >= quota) break;
      const owners = words.filter((word) => word.keys.includes(key));
      if (owners.some((word) => (blanksLeft.get(word) || 0) <= 1)) continue;
      const cell = cells[key];
      cell.fixed = true;
      cell.text = cell.letter.toUpperCase();
      for (const word of owners) {
        blanksLeft.set(word, (blanksLeft.get(word) || 0) - 1);
      }
      revealed += 1;
    }
    return revealed;
  };
  const SEED_COUNT = 6;
  const crossings = cellKeys.filter((key) => ownerCount.get(key) === 2).sort((a, b) => distanceFromCenter(a) - distanceFromCenter(b));
  const singles = cellKeys.filter((key) => ownerCount.get(key) === 1).sort((a, b) => distanceFromCenter(a) - distanceFromCenter(b));
  const seeded = revealSeeds(crossings, 4);
  revealSeeds(singles, SEED_COUNT - seeded);

  // 从左上往右下扫，给词头编号（一个格子同时起横竖两条词时共用一个号）。
  let num = 0;
  for (let y = 0; y < rows; y += 1) {
    for (let x = 0; x < cols; x += 1) {
      const hits = words.filter((word) => word.sx === x && word.sy === y);
      if (!hits.length) continue;
      num += 1;
      for (const hit of hits) {
        hit.number = num;
      }
      cells[K(x, y)].number = num;
    }
  }

  return { cols, rows, cells, words };
}

// ── 查询工具 ────────────────────────────────────────────────────────

function wordsAt(key) {
  const b = board.value;
  return b ? b.words.filter((word) => word.keys.includes(key)) : [];
}

function wordAt(key, dir) {
  const b = board.value;
  return b ? b.words.find((word) => word.dir === dir && word.keys.includes(key)) || null : null;
}

function isLocked(key) {
  // 已经判对或已揭晓的词，字母钉死保护起来。
  return wordsAt(key).some((word) => word.solved || word.givenUp);
}

const currentWord = computed(() => wordAt(selected.value.k, selected.value.dir) || wordAt(selected.value.k, selected.value.dir === "across" ? "down" : "across"));

const activeKeys = computed(() => new Set(currentWord.value ? currentWord.value.keys : []));

const gridList = computed(() => {
  const b = board.value;
  if (!b) return [];
  const out = [];
  for (let y = 0; y < b.rows; y += 1) {
    for (let x = 0; x < b.cols; x += 1) {
      const key = K(x, y);
      out.push({ key, cell: b.cells[key] || null });
    }
  }
  return out;
});

const gridStyle = computed(() => {
  const b = board.value;
  if (!b) return {};
  const size = Math.min(30, Math.max(16, Math.floor(430 / Math.max(b.cols, b.rows))));
  return {
    "--cs": `${size}px`,
    gridTemplateColumns: `repeat(${b.cols}, var(--cs))`,
  };
});

// ── 交互 ────────────────────────────────────────────────────────────

function cellId(key) {
  const [x, y] = parseKey(key);
  return `wallish-cross-${x}-${y}`;
}

function focusKey(key, select) {
  if (!key) return;
  const el = document.getElementById(cellId(key));
  if (!el) return;
  el.focus();
  if (select) el.select();
}

function pickCell(key, toggle) {
  if (!board.value || !board.value.cells[key]) return;
  if (selected.value.k === key && toggle) {
    const other = wordAt(key, selected.value.dir === "across" ? "down" : "across");
    if (other) {
      selected.value.dir = other.dir;
      return;
    }
  }
  selected.value.k = key;
  const across = wordAt(key, "across");
  const down = wordAt(key, "down");
  const prefer = [across, down].find((word) => word && !word.solved && !word.givenUp) || across || down;
  selected.value.dir = prefer ? prefer.dir : "across";
}

function openClue(word) {
  const b = board.value;
  if (!b) return;
  const target = word.keys.find((key) => !b.cells[key].text && !isLocked(key)) || word.keys[0];
  selected.value.k = target;
  selected.value.dir = word.dir;
  nextTick(() => focusKey(target, true));
}

// 某条词全填满时判一次卷子。
function checkWord(word) {
  const b = board.value;
  if (!word || word.solved || word.givenUp) return;
  if (!word.keys.every((key) => b.cells[key].text)) return;
  let all = true;
  for (const key of word.keys) {
    const cell = b.cells[key];
    if (cell.text.toLowerCase() !== cell.letter) {
      all = false;
      if (wordsAt(key).every((other) => other === word || !other.solved)) {
        cell.wrong = true;
      }
    }
  }
  if (all) {
    for (const key of word.keys) {
      b.cells[key].wrong = false;
      b.cells[key].ok = true;
    }
    word.solved = true;
    stats.right += 1;
  } else {
    stats.wrong += 1;
  }
}

function onInput(key, event) {
  const b = board.value;
  if (!b) return;
  const cell = b.cells[key];
  if (!cell || isLocked(key)) return;
  const typed = Array.from(event.target.value || "").reverse().find((ch) => /[a-zA-Z]/.test(ch)) || "";
  cell.text = typed.toUpperCase();
  cell.hint = false;
  event.target.value = cell.text;
  if (cell.text) cell.wrong = false;
  for (const word of wordsAt(key)) {
    checkWord(word);
  }
  if (b.cells[key].ok) return; // 填对了就停在这，别跳走
  const word = currentWord.value;
  if (!word || word.solved || !word.keys.includes(key)) return;
  const index = word.keys.indexOf(key);
  for (let i = index + 1; i < word.keys.length; i += 1) {
    const nextKey = word.keys[i];
    if (!b.cells[nextKey].text && !isLocked(nextKey)) {
      focusKey(nextKey, true);
      return;
    }
  }
}

function onKeydown(key, event) {
  const b = board.value;
  if (!b) return;
  const cell = b.cells[key];
  if (!cell || isLocked(key)) return;
  if (event.key === "Backspace" && !cell.text) {
    const word = currentWord.value;
    if (!word) return;
    const index = word.keys.indexOf(key);
    event.preventDefault();
    for (let i = index - 1; i >= 0; i -= 1) {
      const backKey = word.keys[i];
      const backCell = b.cells[backKey];
      if (isLocked(backKey)) continue;
      backCell.text = "";
      backCell.hint = false;
      focusKey(backKey, true);
      return;
    }
  } else if (event.key === "Enter") {
    const word = currentWord.value;
    if (!word) return;
    const index = word.keys.indexOf(key);
    event.preventDefault();
    for (let i = index + 1; i < word.keys.length; i += 1) {
      const nextKey = word.keys[i];
      if (!b.cells[nextKey].text && !isLocked(nextKey)) {
        focusKey(nextKey, true);
        return;
      }
    }
  }
}

function hintCurrent() {
  const b = board.value;
  const word = currentWord.value;
  if (!b || !word || word.solved || word.givenUp) return;
  const target = word.keys.find((key) => !b.cells[key].text && !isLocked(key));
  if (!target) return;
  const cell = b.cells[target];
  cell.text = cell.letter.toUpperCase();
  cell.hint = true;
  cell.wrong = false;
  checkWord(word);
}

function revealCurrent() {
  const b = board.value;
  const word = currentWord.value;
  if (!b || !word || word.solved || word.givenUp) return;
  for (const key of word.keys) {
    const cell = b.cells[key];
    if (wordsAt(key).some((other) => other !== word && other.solved)) continue;
    cell.text = cell.letter.toUpperCase();
    cell.hint = false;
    cell.wrong = false;
  }
  word.givenUp = true;
}

function acrossClues() {
  return board.value ? board.value.words.filter((word) => word.dir === "across") : [];
}
function downClues() {
  return board.value ? board.value.words.filter((word) => word.dir === "down") : [];
}

function clueText(word) {
  const item = word.item;
  const phone = item.phone ? `/${item.phone}/` : "";
  const line = item.lines[0] || "";
  return [phone, line].filter((part) => part).join(" ") || "（这条词没有释义，听发音猜猜看）";
}

async function deal() {
  loading.value = true;
  error.value = "";
  try {
    const items = await invoke("quiz_words", { count: 8 });
    board.value = buildBoard(items);
    dealSeq.value += 1;
    stats.right = 0;
    stats.wrong = 0;
    selected.value = { k: "", dir: "across" };
    await nextTick();
    const first = board.value.words.find((word) => !word.solved) || board.value.words[0];
    if (first) openClue(first);
  } catch (err) {
    board.value = null;
    error.value = errorText(err, "没有拿到单词");
  } finally {
    loading.value = false;
  }
}

onMounted(deal);
</script>

<template>
  <main class="wallish-cross" :class="{ 'wallish-cross--embedded': props.embedded }">
    <header v-if="!props.embedded" class="wallish-cross__bar">
      <button class="wallish-cross__back" type="button" @click="emit('back')">‹ 返回</button>
      <h1 class="wallish-cross__title">纵横填字</h1>
      <span class="wallish-cross__badge-score">{{ score }}/100</span>
    </header>
    <p class="wallish-cross__intro">{{ levelLabel }}词库随机 8 个词。盘上亮着的字母是给出的提示，其余按横竖线索填满；点两次格子可换方向。</p>

    <div v-if="loading" class="wallish-cross__empty">正在拼盘…</div>
    <template v-else>
      <div v-if="error" class="wallish-cross__empty">
        <p class="wallish-home__error">{{ error }}</p>
        <button class="wallish-cross__tool" type="button" @click="deal">重试</button>
      </div>
      <template v-else-if="board">
        <p v-if="won" class="wallish-cross__won">8 条全通！🎉 换一批再来。</p>
        <div class="wallish-cross__layout">
          <div class="wallish-cross__boardwrap">
            <div class="wallish-cross__board" :style="gridStyle">
              <div
                v-for="entry in gridList"
                :key="`${dealSeq}-${entry.key}`"
                class="wallish-cross__slot"
                :class="{ 'wallish-cross__slot--block': !entry.cell }"
              >
                <template v-if="entry.cell">
                  <span v-if="entry.cell.number" class="wallish-cross__num">{{ entry.cell.number }}</span>
                  <span
                    v-if="entry.cell.fixed || isLocked(entry.key)"
                    class="wallish-cross__letter"
                    :class="{
                      'wallish-cross__letter--ok': wordsAt(entry.key).some((word) => word.solved),
                    }"
                  >{{ entry.cell.text }}</span>
                  <input
                    v-else
                    :id="cellId(entry.key)"
                    class="wallish-cross__input"
                    :class="{
                      'wallish-cross__input--on': activeKeys.has(entry.key),
                      'wallish-cross__input--wrong': entry.cell.wrong,
                      'wallish-cross__input--hint': entry.cell.hint,
                    }"
                    type="text"
                    maxlength="2"
                    autocomplete="off"
                    autocorrect="off"
                    autocapitalize="characters"
                    spellcheck="false"
                    tabindex="-1"
                    :value="entry.cell.text"
                    :aria-label="`格子 ${entry.key}`"
                    @input="onInput(entry.key, $event)"
                    @keydown="onKeydown(entry.key, $event)"
                    @focus="$event.target.select()"
                    @click="pickCell(entry.key, true)"
                  />
                </template>
              </div>
            </div>
          </div>
          <aside class="wallish-cross__stats">
            <div class="wallish-cross__stat">
              <p class="wallish-cross__stat-label">成 绩</p>
              <p class="wallish-cross__stat-value">{{ score }}<small>/100</small></p>
            </div>
            <div class="wallish-cross__stat">
              <p class="wallish-cross__stat-label">答 错</p>
              <p class="wallish-cross__stat-value wallish-cross__stat-value--bad">{{ stats.wrong }}</p>
            </div>
            <div class="wallish-cross__stat">
              <p class="wallish-cross__stat-label">答 对</p>
              <p class="wallish-cross__stat-value wallish-cross__stat-value--good">{{ stats.right }}</p>
            </div>
            <div class="wallish-cross__stat">
              <p class="wallish-cross__stat-label">总字数</p>
              <p class="wallish-cross__stat-value">{{ blankCount }}</p>
            </div>
          </aside>
        </div>

        <div class="wallish-cross__tools">
          <button class="wallish-cross__tool" type="button" :disabled="!currentWord || currentWord.solved || currentWord.givenUp" @click="hintCurrent">
            提示一格
          </button>
          <button class="wallish-cross__tool" type="button" :disabled="!currentWord || currentWord.solved || currentWord.givenUp" @click="revealCurrent">
            揭晓本词
          </button>
          <button class="wallish-cross__tool" type="button" @click="deal">换一批</button>
        </div>

        <div class="wallish-cross__clues">
          <section class="wallish-cross__cluecol">
            <h2 class="wallish-cross__cluetitle">横</h2>
            <button
              v-for="word in acrossClues()"
              :key="`a${word.number}-${dealSeq}`"
              class="wallish-cross__clue"
              :class="{ 'wallish-cross__clue--done': word.solved || word.givenUp }"
              type="button"
              @click="openClue(word)"
            >
              <b>{{ word.number }}</b>. {{ clueText(word) }}<span v-if="word.solved" class="wallish-cross__clueok"> ✓</span>
            </button>
          </section>
          <section class="wallish-cross__cluecol">
            <h2 class="wallish-cross__cluetitle">纵</h2>
            <button
              v-for="word in downClues()"
              :key="`d${word.number}-${dealSeq}`"
              class="wallish-cross__clue"
              :class="{ 'wallish-cross__clue--done': word.solved || word.givenUp }"
              type="button"
              @click="openClue(word)"
            >
              <b>{{ word.number }}</b>. {{ clueText(word) }}<span v-if="word.solved" class="wallish-cross__clueok"> ✓</span>
            </button>
          </section>
        </div>
      </template>
    </template>
  </main>
</template>
