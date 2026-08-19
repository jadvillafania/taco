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
      <button @click="clearAll" :disabled="!entries.length">Clear all</button>
    </div>
    <p v-if="error" class="error">{{ error }}</p>
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
.history { padding: 10px; font-family: system-ui; height: 100vh; box-sizing: border-box; overflow-y: auto; }
.bar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
.empty { color: #666; }
.error { font-size: 12px; color: #c00; margin: 0; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 10px; }
.card img { width: 100%; height: 100px; object-fit: cover; border: 1px solid #ccc; cursor: pointer; }
.meta { display: flex; justify-content: space-between; font-size: 11px; color: #666; }
</style>
