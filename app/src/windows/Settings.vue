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

type HotkeySlot = "region" | "window" | "clipboard";
const recording = ref<HotkeySlot | "">("");
const recordHint = ref("");

const slotRef = (slot: HotkeySlot) =>
  slot === "region" ? hotkeyRegion : slot === "window" ? hotkeyWindow : hotkeyClipboard;

function chordFromEvent(e: KeyboardEvent): string | null | "" {
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return null; // wait for a real key
  let key = "";
  if (e.code.startsWith("Key")) key = e.code.slice(3);
  else if (e.code.startsWith("Digit")) key = e.code.slice(5);
  else if (e.code === "Space") key = "Space";
  else if (/^F([1-9]|1[0-2])$/.test(e.key)) key = e.key;
  else return null; // unsupported key: keep recording
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  if (!mods.length) return ""; // no modifier: rejected
  return [...mods, key].join("+");
}

function onRecordKey(e: KeyboardEvent, slot: HotkeySlot) {
  e.preventDefault();
  if (e.key === "Escape") {
    recording.value = "";
    recordHint.value = "";
    return;
  }
  const chord = chordFromEvent(e);
  if (chord === null) return;
  if (chord === "") {
    recordHint.value = "Add a modifier (Ctrl, Alt, Shift)";
    return;
  }
  slotRef(slot).value = chord;
  recording.value = "";
  recordHint.value = "";
}

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
      <input
        readonly
        :value="recording === 'region' ? 'Press keys…' : hotkeyRegion"
        :class="{ recording: recording === 'region' }"
        @focus="recording = 'region'; recordHint = ''"
        @blur="recording === 'region' && (recording = '')"
        @keydown="onRecordKey($event, 'region')"
      />
    </label>
    <label>
      Active window hotkey
      <input
        readonly
        :value="recording === 'window' ? 'Press keys…' : hotkeyWindow"
        :class="{ recording: recording === 'window' }"
        @focus="recording = 'window'; recordHint = ''"
        @blur="recording === 'window' && (recording = '')"
        @keydown="onRecordKey($event, 'window')"
      />
    </label>
    <label>
      Clipboard image hotkey
      <input
        readonly
        :value="recording === 'clipboard' ? 'Press keys…' : hotkeyClipboard"
        :class="{ recording: recording === 'clipboard' }"
        @focus="recording = 'clipboard'; recordHint = ''"
        @blur="recording === 'clipboard' && (recording = '')"
        @keydown="onRecordKey($event, 'clipboard')"
      />
    </label>
    <p v-if="recordHint" class="hint">{{ recordHint }}</p>
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
input[readonly] { cursor: pointer; background: #fafafa; }
.recording { outline: 2px solid #e11; background: #fff; }
.hint { font-size: 12px; color: #a60; margin: 0; }
.buttons { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; }
.primary { font-weight: 600; }
</style>
