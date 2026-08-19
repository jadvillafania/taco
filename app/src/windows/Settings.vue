<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const retentionHours = ref(24);
const defaultInstruction = ref("");
const hotkeyRegion = ref("");
const hotkeyWindow = ref("");
const hotkeyClipboard = ref("");
const saved = ref(false);
const error = ref("");

onMounted(async () => {
  const s = await invoke<{
    retention_hours: number;
    default_instruction: string;
    hotkey_region: string;
    hotkey_window: string;
    hotkey_clipboard: string;
  }>("get_settings");
  retentionHours.value = s.retention_hours;
  defaultInstruction.value = s.default_instruction;
  hotkeyRegion.value = s.hotkey_region;
  hotkeyWindow.value = s.hotkey_window;
  hotkeyClipboard.value = s.hotkey_clipboard;
});

async function save() {
  error.value = "";
  try {
    await invoke("set_settings", {
      settings: {
        retention_hours: Math.max(1, retentionHours.value),
        default_instruction: defaultInstruction.value,
        hotkey_region: hotkeyRegion.value,
        hotkey_window: hotkeyWindow.value,
        hotkey_clipboard: hotkeyClipboard.value,
      },
    });
    saved.value = true;
    setTimeout(() => getCurrentWindow().close(), 400);
  } catch (e) {
    error.value = String(e);
  }
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
    <label>
      Region capture hotkey
      <input v-model="hotkeyRegion" />
    </label>
    <label>
      Active window hotkey
      <input v-model="hotkeyWindow" />
    </label>
    <label>
      Clipboard image hotkey
      <input v-model="hotkeyClipboard" />
    </label>
    <p v-if="error" class="error">{{ error }}</p>
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
.error { font-size: 12px; color: #c00; margin: 0; }
.buttons { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; }
.primary { font-weight: 600; }
</style>
