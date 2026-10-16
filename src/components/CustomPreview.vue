<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";

const props = defineProps({
  preview: { type: Object, required: true },
  busy: { type: Boolean, default: false },
  error: { type: String, default: "" },
});
const emit = defineEmits(["confirm", "pick", "back"]);

const frameRef = ref(null);
const frame = ref({ width: 0, height: 0 });
const focus = ref({ x: 0.5, y: 0.5 });
let drag = null;

watch(
  () => props.preview.imageDataUrl,
  () => {
    focus.value = { x: 0.5, y: 0.5 };
  },
);

const scale = computed(() =>
  frame.value.width ? Math.max(frame.value.width / props.preview.sourceWidth, frame.value.height / props.preview.sourceHeight) : 1,
);
const imageWidth = computed(() => props.preview.sourceWidth * scale.value);
const imageHeight = computed(() => props.preview.sourceHeight * scale.value);
const extraX = computed(() => Math.max(0, imageWidth.value - frame.value.width));
const extraY = computed(() => Math.max(0, imageHeight.value - frame.value.height));
const canDrag = computed(() => extraX.value > 1 || extraY.value > 1);

function measure() {
  const node = frameRef.value;
  if (!node) return;
  frame.value = { width: node.clientWidth, height: node.clientHeight };
}

function onPointerDown(event) {
  if (!canDrag.value || props.busy) return;
  event.currentTarget.setPointerCapture(event.pointerId);
  drag = { x: event.clientX, y: event.clientY, focusX: focus.value.x, focusY: focus.value.y };
}

function onPointerMove(event) {
  if (!drag) return;
  const nextX = extraX.value ? drag.focusX - (event.clientX - drag.x) / extraX.value : 0.5;
  const nextY = extraY.value ? drag.focusY - (event.clientY - drag.y) / extraY.value : 0.5;
  focus.value = {
    x: Math.min(1, Math.max(0, nextX)),
    y: Math.min(1, Math.max(0, nextY)),
  };
}

function onPointerUp() {
  drag = null;
}

onMounted(() => {
  measure();
  window.addEventListener("resize", measure);
});
onUnmounted(() => window.removeEventListener("resize", measure));
</script>

<template>
  <main class="wallish-custom">
    <header class="wallish-custom__bar">
      <button class="wallish-custom__back" type="button" @click="emit('back')">‹ 返回</button>
      <h1 class="wallish-custom__title">自定义壁纸</h1>
    </header>
    <div
      ref="frameRef"
      :class="canDrag ? 'wallish-custom__frame wallish-custom__frame--drag' : 'wallish-custom__frame'"
      :style="{ aspectRatio: `${preview.targetWidth} / ${preview.targetHeight}` }"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
    >
      <img
        v-if="frame.width"
        class="wallish-custom__shot"
        :src="preview.imageDataUrl"
        alt=""
        draggable="false"
        :style="{
          width: `${imageWidth}px`,
          height: `${imageHeight}px`,
          transform: `translate(${-extraX * focus.x}px, ${-extraY * focus.y}px)`,
        }"
      />
    </div>
    <p class="wallish-custom__name">{{ preview.name }}</p>
    <p class="wallish-custom__hint">裁成 {{ preview.targetWidth }} × {{ preview.targetHeight }}，和每日壁纸一样。拖动画面选择留下的部分。</p>
    <p v-if="error" class="wallish-home__error">{{ error }}</p>
    <div class="wallish-custom__actions">
      <button class="wallish-custom__button" type="button" :disabled="busy" @click="emit('pick')">重新选择</button>
      <button class="wallish-custom__button wallish-custom__button--accent" type="button" :disabled="busy" @click="emit('confirm', focus)">
        确定
      </button>
    </div>
    <p v-if="busy" class="wallish-custom__status">正在设置桌面壁纸…</p>
  </main>
</template>
