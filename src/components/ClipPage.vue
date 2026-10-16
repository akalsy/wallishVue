<script setup>
defineProps({
  word: { type: String, default: "" },
});
const emit = defineEmits(["back"]);
</script>

<template>
  <main class="wallish-clip">
    <header class="wallish-clip__bar">
      <button class="wallish-clip__back" type="button" @click="emit('back')">‹ 返回</button>
      <h1 class="wallish-clip__title">看单词 {{ word }}</h1>
    </header>
  </main>
</template>
