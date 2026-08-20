<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import logo from "../assets/taco.png";

const version = ref("");

onMounted(async () => {
  version.value = await getVersion();
  const w = getCurrentWindow();
  await w.show();
  await w.setFocus();
});

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") getCurrentWindow().close();
}
</script>

<template>
  <div class="about" tabindex="-1" @keydown="onKeydown">
    <div class="body">
      <img class="logo" :src="logo" alt="Taco logo" />
      <div class="id">
        <h1>Taco</h1>
        <p class="expansion"><b>T</b>erminal <b>A</b>gent <b>C</b>ontext <b>O</b>ptics</p>
        <dl>
          <dt>Author</dt>
          <dd>John Arnold Villafania</dd>
          <dt>Build</dt>
          <dd>{{ version }}</dd>
        </dl>
      </div>
    </div>
    <div class="footer">Local-first — screenshots never leave this machine.</div>
  </div>
</template>

<style scoped>
.about {
  height: 100%;
  display: flex;
  flex-direction: column;
  outline: none;
}
.body {
  flex: 1;
  display: flex;
  gap: 26px;
  padding: 30px;
  align-items: flex-start;
}
.logo {
  flex: none;
  width: 72px;
  height: auto;
}
h1 {
  font: 600 32px/1 var(--font-mono);
  letter-spacing: -0.02em;
  margin: 0 0 8px;
}
.expansion {
  font: 500 12.5px/1.5 var(--font-mono);
  letter-spacing: 0.04em;
  color: var(--muted);
  margin: 0 0 22px;
}
.expansion b { color: var(--accent); font-weight: 600; }
dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 6px 18px;
  margin: 0;
  font-size: 13px;
}
dt { color: var(--muted); }
dd { margin: 0; font: 500 12.5px var(--font-mono); padding-top: 1px; }
.footer {
  padding: 10px 30px;
  border-top: 1px solid var(--line);
  font-size: 11.5px;
  color: var(--muted);
}
.footer::before {
  content: "●";
  color: var(--accent);
  font-size: 8px;
  vertical-align: 2px;
  margin-right: 8px;
}
</style>
