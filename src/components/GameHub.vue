<script setup>
import { ref } from "vue";
import WordCross from "./WordCross.vue";
import WordGame from "./WordGame.vue";

defineProps({
  level: { type: String, default: "" },
});
const emit = defineEmits(["back"]);

const tabs = [
  { id: "rows", label: "填词", comp: WordGame },
  { id: "cross", label: "填字", comp: WordCross },
];
const active = ref("rows");
</script>

<template>
  <main class="wallish-hub">
    <header class="wallish-hub__bar">
      <button class="wallish-hub__back" type="button" @click="emit('back')">‹ 返回</button>
      <h1 class="wallish-hub__title">填字游戏</h1>
      <div class="wallish-hub__tabs">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="wallish-hub__tab"
          :class="{ 'wallish-hub__tab--on': active === tab.id }"
          type="button"
          @click="active = tab.id"
        >
          {{ tab.label }}
        </button>
      </div>
    </header>
    <div class="wallish-hub__body">
      <KeepAlive include="WordGame,WordCross">
        <component :is="tabs.find((tab) => tab.id === active).comp" :level="level" embedded />
      </KeepAlive>
    </div>
  </main>
</template>
