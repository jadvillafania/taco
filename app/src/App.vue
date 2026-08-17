<script setup lang="ts">
import Overlay from "./windows/Overlay.vue";
import Composer from "./windows/Composer.vue";
const win = new URLSearchParams(location.search).get("window");
</script>

<template>
  <Overlay v-if="win === 'overlay'" />
  <Composer v-else-if="win === 'composer'" />
</template>

<style>
html,
body,
#app {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
}
</style>
