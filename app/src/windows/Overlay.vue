<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

const frameSrc = ref("");
const drag = ref<{ x: number; y: number } | null>(null);
const rect = ref<{ x: number; y: number; w: number; h: number } | null>(null);

onMounted(async () => {
  // Listen before awaiting the frame: if get_frame rejects, this overlay is still a
  // fullscreen always-on-top window and Escape has to remain able to dismiss it.
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") invoke("cancel_overlay");
  });
  try {
    frameSrc.value = convertFileSrc(await invoke<string>("get_frame"));
  } catch {
    invoke("cancel_overlay");
  }
});

function down(e: MouseEvent) {
  drag.value = { x: e.clientX, y: e.clientY };
  rect.value = { x: e.clientX, y: e.clientY, w: 0, h: 0 };
}
function move(e: MouseEvent) {
  if (!drag.value) return;
  rect.value = {
    x: Math.min(drag.value.x, e.clientX),
    y: Math.min(drag.value.y, e.clientY),
    w: Math.abs(e.clientX - drag.value.x),
    h: Math.abs(e.clientY - drag.value.y),
  };
}
function up() {
  const r = rect.value;
  drag.value = null;
  if (!r || r.w < 4 || r.h < 4) { rect.value = null; return; }
  const s = window.devicePixelRatio;
  invoke("region_selected", {
    x: Math.round(r.x * s), y: Math.round(r.y * s),
    w: Math.round(r.w * s), h: Math.round(r.h * s),
  });
}
</script>

<template>
  <div class="overlay" @mousedown="down" @mousemove="move" @mouseup="up">
    <img :src="frameSrc" class="frame" draggable="false" />
    <div class="dim" />
    <div v-if="rect" class="sel"
      :style="{ left: rect.x + 'px', top: rect.y + 'px', width: rect.w + 'px', height: rect.h + 'px' }">
      <div class="cut">
        <img :src="frameSrc" class="frame"
          :style="{ left: -rect.x + 'px', top: -rect.y + 'px' }" draggable="false" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay { position: fixed; inset: 0; cursor: crosshair; overflow: hidden; user-select: none; }
.frame { position: absolute; left: 0; top: 0; width: 100vw; height: 100vh; }
.dim { position: absolute; inset: 0; background: rgba(21, 23, 28, 0.45); }
.sel { position: absolute; overflow: visible; outline: 1.5px solid #F2A33C; }
.sel::before, .sel::after {
  content: ""; position: absolute; width: 14px; height: 14px;
  border: 2.5px solid #F2A33C; pointer-events: none;
}
.sel::before { top: -3px; left: -3px; border-right: none; border-bottom: none; }
.sel::after { bottom: -3px; right: -3px; border-left: none; border-top: none; }
.cut { position: absolute; inset: 0; overflow: hidden; }
</style>
