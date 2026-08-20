<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const retentionHours = ref(24);
const defaultInstruction = ref("");
const hotkeyRegion = ref("");
const hotkeyWindow = ref("");
const hotkeyClipboard = ref("");
const saved = ref(false);
const error = ref("");
const loaded = ref("");

type HotkeySlot = "region" | "window" | "clipboard";
const recording = ref<HotkeySlot | "">("");
const recordHint = ref("");
const probeMsg = ref<Partial<Record<HotkeySlot, string>>>({});

const SLOT_LABELS: Record<HotkeySlot, string> = {
  region: "Region capture",
  window: "Active window",
  clipboard: "Clipboard image",
};

type ProbeVerdict = { level: "ok" | "warn" | "block"; message: string };
let probing = false;

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
  if (recording.value !== slot) return;
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
  for (const other of ["region", "window", "clipboard"] as HotkeySlot[]) {
    if (other !== slot && slotRef(other).value === chord) {
      slotRef(slot).value = chord;
      probeMsg.value = { ...probeMsg.value, [slot]: `already used by ${SLOT_LABELS[other]} (unsaved)` };
      recording.value = "";
      recordHint.value = "";
      return;
    }
  }

  if (probing) return;
  probing = true;
  invoke<ProbeVerdict>("probe_hotkey", { binding: chord, exclude: slot })
    .then((v) => {
      if (v.level === "block") {
        recordHint.value = `${v.message} — try a different combination`;
        return; // stay recording; ref untouched
      }
      slotRef(slot).value = chord;
      probeMsg.value = { ...probeMsg.value, [slot]: v.level === "warn" ? v.message : "" };
      recording.value = "";
      recordHint.value = "";
    })
    .catch((e) => { recordHint.value = String(e); })
    .finally(() => { probing = false; });
}

function onRecordBlur(slot: HotkeySlot) {
  if (recording.value === slot) recording.value = "";
  recordHint.value = "";
}

function snapshot() {
  return JSON.stringify([
    retentionHours.value, defaultInstruction.value,
    hotkeyRegion.value, hotkeyWindow.value, hotkeyClipboard.value,
  ]);
}
const dirty = computed(() => loaded.value !== "" && snapshot() !== loaded.value);

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
  loaded.value = snapshot();
  try {
    const w = getCurrentWindow();
    await w.show();
    await w.setFocus();
  } catch {
    /* tray re-click force-shows */
  }
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
    <label class="field-label">
      Keep captures for (hours)
      <input class="input" type="number" min="1" v-model.number="retentionHours" />
    </label>
    <label class="field-label">
      Default instruction (when message is empty)
      <textarea class="textarea" rows="3" v-model="defaultInstruction" />
    </label>
    <label class="field-label">
      Region capture hotkey
      <input
        class="input"
        readonly
        :value="recording === 'region' ? 'Press keys…' : hotkeyRegion"
        :class="{ recording: recording === 'region' }"
        @focus="recording = 'region'; recordHint = ''; probeMsg = { ...probeMsg, region: '' }"
        @blur="onRecordBlur('region')"
        @keydown="onRecordKey($event, 'region')"
      />
    </label>
    <p v-if="probeMsg.region" class="hint">{{ probeMsg.region }}</p>
    <label class="field-label">
      Active window hotkey
      <input
        class="input"
        readonly
        :value="recording === 'window' ? 'Press keys…' : hotkeyWindow"
        :class="{ recording: recording === 'window' }"
        @focus="recording = 'window'; recordHint = ''; probeMsg = { ...probeMsg, window: '' }"
        @blur="onRecordBlur('window')"
        @keydown="onRecordKey($event, 'window')"
      />
    </label>
    <p v-if="probeMsg.window" class="hint">{{ probeMsg.window }}</p>
    <label class="field-label">
      Clipboard image hotkey
      <input
        class="input"
        readonly
        :value="recording === 'clipboard' ? 'Press keys…' : hotkeyClipboard"
        :class="{ recording: recording === 'clipboard' }"
        @focus="recording = 'clipboard'; recordHint = ''; probeMsg = { ...probeMsg, clipboard: '' }"
        @blur="onRecordBlur('clipboard')"
        @keydown="onRecordKey($event, 'clipboard')"
      />
    </label>
    <p v-if="probeMsg.clipboard" class="hint">{{ probeMsg.clipboard }}</p>
    <p v-if="recordHint" class="hint">{{ recordHint }}</p>
    <p v-if="error" class="error-text">{{ error }}</p>
    <div class="buttons">
      <button class="btn btn-primary" :disabled="!dirty" @click="save">{{ saved ? "Saved" : "Save" }}</button>
    </div>
  </div>
</template>

<style scoped>
.settings { display: flex; flex-direction: column; gap: 12px; padding: 14px; font-family: var(--font-sans); background: var(--bg); height: 100vh; box-sizing: border-box; }
label { display: flex; flex-direction: column; gap: 5px; }
.settings input, .settings textarea { text-transform: none; letter-spacing: normal; }
input[readonly] { cursor: pointer; background: var(--raised); font-family: var(--font-mono); font-size: 12px; }
.recording { outline: 2px solid var(--accent); background: var(--raised); }
.hint { font-size: 12px; color: var(--accent); margin: 0; }
.buttons { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; }
</style>
