<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

type Entry = { path: string; name: string; modified: number };
const entries = ref<Entry[]>([]);
const error = ref("");

async function refresh() {
  entries.value = await invoke<Entry[]>("list_captures");
}
onMounted(refresh);

async function resend(e: Entry) {
  error.value = "";
  try {
    await invoke("resend_capture", { path: e.path });
  } catch (err) {
    error.value = String(err);
  }
}
async function remove(e: Entry) {
  error.value = "";
  try {
    await invoke("delete_capture", { path: e.path });
    await refresh();
  } catch (err) {
    error.value = String(err);
  }
}
async function clearAll() {
  if (!confirm("Delete all captures?")) return;
  error.value = "";
  try {
    await invoke("clear_captures");
    await refresh();
  } catch (err) {
    error.value = String(err);
  }
}
function stamp(e: Entry) {
  return new Date(e.modified * 1000).toLocaleString();
}
</script>

<template>
  <div class="history">
    <div class="bar">
      <span>{{ entries.length }} captures</span>
      <button class="btn btn-danger" @click="clearAll" :disabled="!entries.length">Clear all</button>
    </div>
    <p v-if="error" class="error-text">{{ error }}</p>
    <p v-if="!entries.length" class="empty">No captures.</p>
    <div class="grid">
      <div v-for="e in entries" :key="e.path" class="card">
        <img :src="convertFileSrc(e.path)" @click="resend(e)" :title="'Resend ' + e.name" />
        <div class="meta">
          <span>{{ stamp(e) }}</span>
          <button @click="remove(e)">Delete</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.history { padding: 14px; font-family: var(--font-sans); background: var(--bg); height: 100vh; box-sizing: border-box; overflow-y: auto; }
.bar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; font-size: 12px; color: var(--muted); }
.empty { color: var(--muted); }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 10px; }
.card { border: 1px solid var(--line); border-radius: 8px; overflow: hidden; background: var(--raised); }
.card img { width: 100%; height: 100px; object-fit: cover; cursor: pointer; display: block; border: none; }
.meta { display: flex; justify-content: space-between; align-items: center; padding: 6px 9px; font: 500 11px var(--font-mono); color: var(--muted); }
.meta button { background: transparent; border: none; color: var(--danger); font: 500 11px var(--font-sans); cursor: pointer; padding: 0; }
.btn-danger { font-size: 12px; padding: 4px 10px; }
</style>
