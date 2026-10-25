<script setup>
import { computed, ref } from "vue";

const props = defineProps({
  imageUrl: { type: String, default: "" },
  box: { type: Object, required: true },
  level: { type: String, default: "cet4" },
  wallpaperSource: { type: String, default: "auto" },
  wordRefresh: { type: Number, default: 30 },
  autostart: { type: Boolean, default: false },
  saving: { type: Boolean, default: false },
  error: { type: String, default: "" },
});
const emit = defineEmits(["save", "back"]);

const positions = [
  { id: "top", label: "靠上" },
  { id: "center", label: "居中" },
  { id: "bottom", label: "靠下" },
  { id: "left", label: "靠左" },
  { id: "right", label: "靠右" },
];
const sizes = [
  { id: "small", label: "小" },
  { id: "medium", label: "中" },
  { id: "large", label: "大" },
];
const refreshes = [
  { id: 0, label: "关闭" },
  { id: 15, label: "15 分钟" },
  { id: 30, label: "30 分钟" },
  { id: 60, label: "1 小时" },
];
const levels = [
  { id: "cet4", label: "四级" },
  { id: "cet6", label: "六级" },
  { id: "kaoyan", label: "考研" },
  { id: "ielts", label: "雅思" },
  { id: "toefl", label: "托福" },
  { id: "gre", label: "GRE" },
  { id: "jp-n5", label: "日语N5" },
  { id: "jp-n4", label: "日语N4" },
  { id: "jp-n3", label: "日语N3" },
  { id: "jp-n2", label: "日语N2" },
  { id: "jp-n1", label: "日语N1" },
];
const sources = [
  { id: "auto", label: "自动", proxy: false },
  { id: "bing", label: "必应", proxy: false },
  { id: "photo", label: "风景图", proxy: false },
  { id: "anime", label: "二次元", proxy: false },
  { id: "picsum", label: "随拍", proxy: false },
  { id: "wallhaven", label: "壁纸站", proxy: true },
];

const draft = ref({ ...props.box, level: props.level, wallpaperSource: props.wallpaperSource, wordRefresh: props.wordRefresh, autostart: props.autostart });
const hidden = computed(() => draft.value.caption === false);
const dirty = computed(() => {
  const left = draft.value;
  const right = props.box;
  return (
    left.position !== right.position ||
    left.background !== right.background ||
    left.size !== right.size ||
    (left.caption !== false) !== (right.caption !== false) ||
    left.level !== props.level ||
    left.wallpaperSource !== props.wallpaperSource ||
    Number(left.wordRefresh) !== Number(props.wordRefresh) ||
    Boolean(left.autostart) !== Boolean(props.autostart)
  );
});

function patch(part) {
  draft.value = { ...draft.value, ...part };
}
</script>

<template>
  <main class="wallish-settings">
    <header class="wallish-settings__bar">
      <button class="wallish-settings__back" type="button" @click="emit('back')">‹ 返回</button>
      <h1 class="wallish-settings__title">设置</h1>
    </header>
    <div class="wallish-settings__body">
      <div class="wallish-settings__preview">
        <img v-if="imageUrl" class="wallish-home__photo" :src="imageUrl" alt="" />
        <div v-else class="wallish-home__photo wallish-home__photo--empty">壁纸预览</div>
      </div>
      <div class="wallish-settings__panel">
        <p class="wallish-settings__hint">改完要点保存，才会换掉桌面壁纸。不显示词条时，桌面只留照片。</p>
        <p class="wallish-settings__label">单词等级</p>
        <div class="wallish-settings__sizes">
          <button
            v-for="item in levels"
            :key="item.id"
            :class="draft.level === item.id ? 'wallish-settings__size wallish-settings__size--on' : 'wallish-settings__size'"
            type="button"
            @click="patch({ level: item.id })"
          >
            {{ item.label }}
          </button>
        </div>
        <p class="wallish-settings__label">词条自动更新</p>
        <div class="wallish-settings__sizes">
          <button
            v-for="item in refreshes"
            :key="item.id"
            :class="Number(draft.wordRefresh) === item.id ? 'wallish-settings__size wallish-settings__size--on' : 'wallish-settings__size'"
            type="button"
            @click="patch({ wordRefresh: item.id })"
          >
            {{ item.label }}
          </button>
        </div>
        <p class="wallish-settings__hint">到点自动换一个新词（桌面壁纸上的单词会更新，需应用保持运行）。</p>
        <p class="wallish-settings__label">壁纸来源</p>
        <div class="wallish-settings__sizes">
          <button
            v-for="item in sources"
            :key="item.id"
            :class="draft.wallpaperSource === item.id ? 'wallish-settings__size wallish-settings__size--on' : 'wallish-settings__size'"
            type="button"
            @click="patch({ wallpaperSource: item.id })"
          >
            {{ item.label }}
            <span v-if="item.proxy" class="wallish-settings__proxy">需代理</span>
          </button>
        </div>
        <p class="wallish-settings__hint">标「需代理」的源（壁纸站）需开着代理才能用；其余可直连。自动=必应→风景图→二次元→壁纸站→随拍，谁先成功用谁。改完下次换壁纸生效。</p>
        <label class="wallish-settings__toggle">
          <span>不显示词条</span>
          <input
            class="wallish-settings__check"
            type="checkbox"
            :checked="hidden"
            @change="patch({ caption: !$event.target.checked })"
          />
          <span :class="hidden ? 'wallish-settings__switch wallish-settings__switch--on' : 'wallish-settings__switch'" />
        </label>
        <template v-if="!hidden">
          <p class="wallish-settings__label">单词条位置</p>
          <div class="wallish-settings__places">
            <button
              v-for="item in positions"
              :key="item.id"
              :class="draft.position === item.id ? 'wallish-settings__place wallish-settings__place--on' : 'wallish-settings__place'"
              type="button"
              @click="patch({ position: item.id })"
            >
              <span :class="`wallish-settings__mark wallish-settings__mark--${item.id}`" />
              <span>{{ item.label }}</span>
            </button>
          </div>
          <label class="wallish-settings__color">
            <span class="wallish-settings__label">背景颜色</span>
            <input class="wallish-settings__picker" type="color" :value="draft.background" @input="patch({ background: $event.target.value })" />
            <span class="wallish-settings__hex">{{ String(draft.background || "").toUpperCase() }}</span>
          </label>
          <p class="wallish-settings__label">单词条大小</p>
          <div class="wallish-settings__sizes">
            <button
              v-for="item in sizes"
              :key="item.id"
              :class="draft.size === item.id ? 'wallish-settings__size wallish-settings__size--on' : 'wallish-settings__size'"
              type="button"
              @click="patch({ size: item.id })"
            >
              {{ item.label }}
            </button>
          </div>
        </template>
        <label class="wallish-settings__toggle">
          <span>开机自动启动</span>
          <input
            class="wallish-settings__check"
            type="checkbox"
            :checked="Boolean(draft.autostart)"
            @change="patch({ autostart: $event.target.checked })"
          />
          <span :class="draft.autostart ? 'wallish-settings__switch wallish-settings__switch--on' : 'wallish-settings__switch'" />
        </label>
        <p class="wallish-settings__hint">开机后在后台自动运行（不弹窗口），方便每天自动换壁纸；从托盘菜单可以打开或退出。</p>
      </div>
    </div>
    <div class="wallish-settings__actions">
      <button class="wallish-settings__save" type="button" :disabled="!dirty || saving" @click="emit('save', draft)">保存</button>
    </div>
    <p v-if="saving" class="wallish-settings__status">正在更新桌面壁纸…</p>
    <p v-if="error" class="wallish-home__error">{{ error }}</p>
  </main>
</template>
