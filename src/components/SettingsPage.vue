<script setup>
import { computed, ref } from "vue";

const props = defineProps({
  imageUrl: { type: String, default: "" },
  box: { type: Object, required: true },
  level: { type: String, default: "cet4" },
  wallpaperSource: { type: String, default: "auto" },
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
const levels = [
  { id: "cet4", label: "四级" },
  { id: "cet6", label: "六级" },
  { id: "kaoyan", label: "考研" },
  { id: "ielts", label: "雅思" },
  { id: "toefl", label: "托福" },
  { id: "gre", label: "GRE" },
];
const sources = [
  { id: "auto", label: "自动" },
  { id: "wallhaven", label: "壁纸站" },
  { id: "bing", label: "必应" },
  { id: "picsum", label: "随拍" },
];

const draft = ref({ ...props.box, level: props.level, wallpaperSource: props.wallpaperSource });
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
    left.wallpaperSource !== props.wallpaperSource
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
          </button>
        </div>
        <p class="wallish-settings__hint">自动=壁纸站→必应→随拍依次尝试；壁纸站(Wallhaven)与随拍(Picsum)可能需科学上网。改完下次换壁纸生效。</p>
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
      </div>
    </div>
    <div class="wallish-settings__actions">
      <button class="wallish-settings__save" type="button" :disabled="!dirty || saving" @click="emit('save', draft)">保存</button>
    </div>
    <p v-if="saving" class="wallish-settings__status">正在更新桌面壁纸…</p>
    <p v-if="error" class="wallish-home__error">{{ error }}</p>
  </main>
</template>
