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
const origHotkeys = ref<Record<HotkeySlot, string>>({ region: "", window: "", clipboard: "" });
const defaults = ref<Record<HotkeySlot, string>>({ region: "", window: "", clipboard: "" });

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
  probing = true; // shared across slots: local IPC resolves in ms, per-slot flags aren't worth it
  invoke<ProbeVerdict>("probe_hotkey", { binding: chord, exclude: slot })
    .then((v) => {
      if (recording.value !== slot) return; // cancelled (Escape/blur) while probing — discard verdict
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

function cancelRecording(slot: HotkeySlot) {
  if (recording.value === slot) recording.value = "";
  recordHint.value = "";
}
function revertSlot(slot: HotkeySlot) {
  slotRef(slot).value = origHotkeys.value[slot];
  probeMsg.value = { ...probeMsg.value, [slot]: "" };
}

function resetHotkeys() {
  (["region", "window", "clipboard"] as HotkeySlot[]).forEach((s) => {
    slotRef(s).value = defaults.value[s];
  });
  probeMsg.value = {};
  recordHint.value = "";
  if (recording.value) recording.value = "";
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
  origHotkeys.value = { region: s.hotkey_region, window: s.hotkey_window, clipboard: s.hotkey_clipboard };
  loaded.value = snapshot();
  const d = await invoke<{ hotkey_region: string; hotkey_window: string; hotkey_clipboard: string }>("get_default_settings");
  defaults.value = { region: d.hotkey_region, window: d.hotkey_window, clipboard: d.hotkey_clipboard };
  try {
    const w = getCurrentWindow();
    await w.show();
    await w.setFocus();
  } catch {
    /* tray re-click force-shows */
  }
});

type ShimHost = "native" | "wsl";
const shimBusy = ref<ShimHost | "">("");
const shimMsg = ref<Partial<Record<ShimHost, string>>>({});

async function shimAction(host: ShimHost, action: "install" | "remove") {
  shimBusy.value = host;
  shimMsg.value = { ...shimMsg.value, [host]: "" };
  const cmd = `${action}_${host === "wsl" ? "wsl" : "native"}_shim`;
  try {
    // ()-returning commands yield null; install_native_shim may yield a warning
    const warn = await invoke<string | null>(cmd);
    const base = action === "install"
      ? "Installed — restart your terminal, then run 'claude' as usual."
      : "Removed — profile wrapper deleted.";
    shimMsg.value = { ...shimMsg.value, [host]: warn ? `${base} ⚠ ${warn}` : base };
  } catch (e) {
    shimMsg.value = { ...shimMsg.value, [host]: String(e) };
  } finally {
    shimBusy.value = "";
  }
}

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
      <div class="hotkey-wrap">
        <input
          class="input"
          readonly
          :value="recording === 'region' ? 'Press keys…' : hotkeyRegion"
          :class="{ recording: recording === 'region' }"
          @focus="recording = 'region'; recordHint = ''; probeMsg = { ...probeMsg, region: '' }"
          @blur="onRecordBlur('region')"
          @keydown="onRecordKey($event, 'region')"
        />
        <button
          v-if="recording === 'region' || hotkeyRegion !== origHotkeys.region"
          class="field-btn"
          :title="recording === 'region' ? 'Cancel recording' : 'Revert to saved'"
          @mousedown.prevent
          @click="recording === 'region' ? cancelRecording('region') : revertSlot('region')"
        >{{ recording === 'region' ? '✕' : '↺' }}</button>
      </div>
    </label>
    <p v-if="probeMsg.region" class="hint">{{ probeMsg.region }}</p>
    <label class="field-label">
      Active window hotkey
      <div class="hotkey-wrap">
        <input
          class="input"
          readonly
          :value="recording === 'window' ? 'Press keys…' : hotkeyWindow"
          :class="{ recording: recording === 'window' }"
          @focus="recording = 'window'; recordHint = ''; probeMsg = { ...probeMsg, window: '' }"
          @blur="onRecordBlur('window')"
          @keydown="onRecordKey($event, 'window')"
        />
        <button
          v-if="recording === 'window' || hotkeyWindow !== origHotkeys.window"
          class="field-btn"
          :title="recording === 'window' ? 'Cancel recording' : 'Revert to saved'"
          @mousedown.prevent
          @click="recording === 'window' ? cancelRecording('window') : revertSlot('window')"
        >{{ recording === 'window' ? '✕' : '↺' }}</button>
      </div>
    </label>
    <p v-if="probeMsg.window" class="hint">{{ probeMsg.window }}</p>
    <label class="field-label">
      Clipboard image hotkey
      <div class="hotkey-wrap">
        <input
          class="input"
          readonly
          :value="recording === 'clipboard' ? 'Press keys…' : hotkeyClipboard"
          :class="{ recording: recording === 'clipboard' }"
          @focus="recording = 'clipboard'; recordHint = ''; probeMsg = { ...probeMsg, clipboard: '' }"
          @blur="onRecordBlur('clipboard')"
          @keydown="onRecordKey($event, 'clipboard')"
        />
        <button
          v-if="recording === 'clipboard' || hotkeyClipboard !== origHotkeys.clipboard"
          class="field-btn"
          :title="recording === 'clipboard' ? 'Cancel recording' : 'Revert to saved'"
          @mousedown.prevent
          @click="recording === 'clipboard' ? cancelRecording('clipboard') : revertSlot('clipboard')"
        >{{ recording === 'clipboard' ? '✕' : '↺' }}</button>
      </div>
    </label>
    <p v-if="probeMsg.clipboard" class="hint">{{ probeMsg.clipboard }}</p>
    <div class="field-label">
      <div class="shim-row">
        <div class="shim-text">
          <strong>Windows (native)</strong>
          <small>Install for Windows PowerShell. Reversible; cmd.exe keeps using clipboard delivery.</small>
        </div>
        <button class="field-btn" :disabled="shimBusy !== ''" @click="shimAction('native', 'install')">Install</button>
        <button class="field-btn" :disabled="shimBusy !== ''" @click="shimAction('native', 'remove')">Remove</button>
      </div>
      <p v-if="shimMsg.native" class="hint">{{ shimMsg.native }}</p>
      <div class="shim-row">
        <div class="shim-text">
          <strong>WSL</strong>
          <small>Install for your WSL distro. Reversible; also enables WSL session discovery.</small>
        </div>
        <button class="field-btn" :disabled="shimBusy !== ''" @click="shimAction('wsl', 'install')">Install</button>
        <button class="field-btn" :disabled="shimBusy !== ''" @click="shimAction('wsl', 'remove')">Remove</button>
      </div>
      <p v-if="shimMsg.wsl" class="hint">{{ shimMsg.wsl }}</p>
    </div>
    <button
      v-if="hotkeyRegion !== defaults.region || hotkeyWindow !== defaults.window || hotkeyClipboard !== defaults.clipboard"
      class="link-btn"
      @click="resetHotkeys"
    >Reset hotkeys to defaults</button>
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
.hint { font-size: 12px; color: var(--muted); margin: 0; }
.buttons { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; }
.hotkey-wrap { position: relative; }
.hotkey-wrap input { width: 100%; padding-right: 30px; box-sizing: border-box; }
.field-btn { position: absolute; right: 4px; top: 50%; transform: translateY(-50%); background: transparent; border: none; color: var(--muted); cursor: pointer; font-size: 13px; padding: 2px 5px; }
.field-btn:hover { color: var(--accent); }
.link-btn { background: transparent; border: none; color: var(--muted); font: 500 12px var(--font-sans); cursor: pointer; padding: 0; text-align: left; }
.link-btn:hover { color: var(--accent); }
/* Description takes the full width; the buttons wrap onto their own row beneath it,
   so the long explanatory text isn't squeezed into a narrow column. */
.shim-row { display: flex; flex-wrap: wrap; gap: 4px 8px; align-items: center; margin-bottom: 10px; }
.shim-text { flex: 1 1 100%; display: flex; flex-direction: column; }
.shim-text small { color: var(--muted); font-size: 11px; }
/* .field-btn is absolutely positioned for the hotkey-wrap context; reset that
   here so it renders as a normal inline button inside the flex shim-row. */
.shim-row .field-btn { position: static; transform: none; }
</style>
