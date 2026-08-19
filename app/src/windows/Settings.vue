<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const retentionHours = ref(24);
const defaultInstruction = ref("");
const saved = ref(false);

onMounted(async () => {
  const s = await invoke<{ retention_hours: number; default_instruction: string }>("get_settings");
  retentionHours.value = s.retention_hours;
  defaultInstruction.value = s.default_instruction;
});

async function save() {
  await invoke("set_settings", {
    settings: { retention_hours: Math.max(1, retentionHours.value), default_instruction: defaultInstruction.value },
  });
  saved.value = true;
  setTimeout(() => getCurrentWindow().close(), 400);
}
</script>

<template>
  <div class="settings">
    <label>
      Keep captures for (hours)
      <input type="number" min="1" v-model.number="retentionHours" />
    </label>
    <label>
      Default instruction (when message is empty)
      <textarea rows="3" v-model="defaultInstruction" />
    </label>
    <div class="buttons">
      <button @click="getCurrentWindow().close()">Cancel</button>
      <button class="primary" @click="save">{{ saved ? "Saved" : "Save" }}</button>
    </div>
  </div>
</template>

<style scoped>
.settings { display: flex; flex-direction: column; gap: 12px; padding: 14px; font-family: system-ui; height: 100vh; box-sizing: border-box; }
label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
textarea { resize: none; }
.buttons { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; }
.primary { font-weight: 600; }
</style>
