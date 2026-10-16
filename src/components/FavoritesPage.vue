<script setup>
defineProps({
  items: { type: Array, default: () => [] },
  busy: { type: Boolean, default: false },
  error: { type: String, default: "" },
});
const emit = defineEmits(["use", "remove", "back"]);
</script>

<template>
  <main class="wallish-favorites">
    <header class="wallish-favorites__bar">
      <button class="wallish-favorites__back" type="button" @click="emit('back')">‹ 返回</button>
      <h1 class="wallish-favorites__title">收藏</h1>
    </header>
    <div v-if="items.length" class="wallish-favorites__list">
      <article v-for="item in items" :key="item.id" class="wallish-favorites__card">
        <button class="wallish-favorites__pick" type="button" :disabled="busy" @click="emit('use', item.id)">
          <img v-if="item.thumbUrl" class="wallish-favorites__thumb" :src="item.thumbUrl" alt="" />
          <span v-else class="wallish-favorites__thumb wallish-favorites__thumb--empty">无预览</span>
          <p class="wallish-favorites__word">{{ item.word || "自定义壁纸" }}</p>
        </button>
        <button class="wallish-favorites__remove" type="button" :disabled="busy" @click="emit('remove', item.id)">取消收藏</button>
      </article>
    </div>
    <p v-else class="wallish-favorites__empty">还没有收藏。先在主页收藏当前壁纸。</p>
    <p v-if="error" class="wallish-home__error">{{ error }}</p>
  </main>
</template>
