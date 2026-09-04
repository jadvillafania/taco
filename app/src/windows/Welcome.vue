<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import logo from "../assets/taco.png";

const keys = ref({ region: "", window: "", clipboard: "" });
const retention = ref(24);
// "" follows WSL's own default; anything else is an explicit pick, applied on change
const wslDistro = ref("");
const distros = ref<string[]>([]);
const wslDefault = ref("");

type ShimHost = "native" | "wsl";
const shimBusy = ref<ShimHost | "">("");
const shimMsg = ref<Partial<Record<ShimHost, string>>>({});
const shimOn = ref<Record<ShimHost, boolean>>({ native: false, wsl: false });

onMounted(async () => {
  const s = await invoke<{
    hotkey_region: string; hotkey_window: string; hotkey_clipboard: string;
    retention_hours: number; wsl_distro: string;
  }>("get_settings");
  keys.value = { region: s.hotkey_region, window: s.hotkey_window, clipboard: s.hotkey_clipboard };
  retention.value = s.retention_hours;
  try {
    shimOn.value = await invoke<Record<ShimHost, boolean>>("shim_status");
  } catch { /* leave both off rather than lock the buttons */ }
  try {
    const d = await invoke<{ distros: string[]; default: string }>("list_distros");
    distros.value = d.distros;
    wslDefault.value = d.default;
    wslDistro.value = s.wsl_distro;
  } catch { /* no WSL installed: the picker just stays empty */ }
  const w = getCurrentWindow();
  await w.show();
  await w.setFocus();
});

async function applyDistro() {
  shimMsg.value = { ...shimMsg.value, wsl: "" };
  try {
    await invoke("set_wsl_distro", { distro: wslDistro.value });
    shimOn.value = await invoke<Record<ShimHost, boolean>>("shim_status");
  } catch (e) {
    shimMsg.value = { ...shimMsg.value, wsl: String(e) };
  }
}

async function install(host: ShimHost) {
  shimBusy.value = host;
  shimMsg.value = { ...shimMsg.value, [host]: "" };
  try {
    // install_native_shim may return a warning string; install_wsl_shim yields null
    const warn = await invoke<string | null>(`install_${host}_shim`);
    const base = "Installed — restart your terminal, then run 'claude' as usual.";
    shimMsg.value = { ...shimMsg.value, [host]: warn ? `${base} ⚠ ${warn}` : base };
  } catch (e) {
    shimMsg.value = { ...shimMsg.value, [host]: String(e) };
  } finally {
    shimBusy.value = "";
    try { shimOn.value = await invoke<Record<ShimHost, boolean>>("shim_status"); } catch { /* keep last */ }
  }
}

async function tryCapture() {
  await invoke("trigger_capture", { kind: "region" });
  getCurrentWindow().close();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") getCurrentWindow().close();
}
</script>

<template>
  <div class="welcome" tabindex="-1" @keydown="onKeydown">
    <header>
      <img class="logo" :src="logo" alt="" />
      <div>
        <h1>Taco is running</h1>
        <p class="expansion"><b>T</b>erminal <b>A</b>gent <b>C</b>ontext <b>O</b>ptics</p>
        <p class="sub">
          Screenshots into your already-running Claude Code session — in WSL or PowerShell.
          Not an AI app: no chat, no model, no account. It only feeds visual context into the
          agent conversation you already have open.
        </p>
        <p class="sub">It lives in your system tray — no window to keep open. Right-click the icon for everything below.</p>
      </div>
    </header>

    <ol class="steps">
      <li>
        <h2>Capture</h2>
        <dl>
          <dt>Region</dt><dd class="mono">{{ keys.region }}</dd>
          <dt>Active window</dt><dd class="mono">{{ keys.window }}</dd>
          <dt>Clipboard image</dt><dd class="mono">{{ keys.clipboard }}</dd>
        </dl>
        <p class="sub">Change these in Settings.</p>
      </li>
      <li>
        <h2>Send</h2>
        <p class="sub">
          The composer opens: type a message, pick the Claude Code session, hit Send. The screenshot goes
          in as a file path your agent can read.
        </p>
      </li>
      <li>
        <h2>Connect your terminal <small>optional</small></h2>
        <p class="sub">
          Installing the shim lets Taco type into a running <span class="mono">claude</span> session directly.
          Without it, Taco copies the message to your clipboard and you paste it. Reversible in Settings.
        </p>
        <div class="row">
          <select
            v-if="distros.length"
            class="selectbox distro"
            v-model="wslDistro"
            @change="applyDistro"
            :disabled="shimBusy !== ''"
            aria-label="WSL distribution"
          >
            <option value="">WSL default{{ wslDefault ? ` (${wslDefault})` : "" }}</option>
            <option v-for="d in distros" :key="d" :value="d">{{ d }}</option>
          </select>
          <button class="btn btn-quiet" :disabled="shimBusy !== '' || shimOn.wsl" @click="install('wsl')">
            {{ shimOn.wsl ? "WSL ✓" : "Install for WSL" }}
          </button>
          <button class="btn btn-quiet" :disabled="shimBusy !== '' || shimOn.native" @click="install('native')">
            {{ shimOn.native ? "PowerShell ✓" : "Install for PowerShell" }}
          </button>
        </div>
        <p v-if="shimMsg.wsl" class="hint">{{ shimMsg.wsl }}</p>
        <p v-if="shimMsg.native" class="hint">{{ shimMsg.native }}</p>
      </li>
    </ol>

    <footer>
      <span class="hint">Screenshots stay on this machine and are deleted after {{ retention }}h.</span>
      <button class="btn btn-quiet" @click="getCurrentWindow().close()">Close</button>
      <button class="btn btn-primary" @click="tryCapture">Try a capture</button>
    </footer>
  </div>
</template>

<style scoped>
.welcome {
  height: 100vh; box-sizing: border-box; display: flex; flex-direction: column;
  background: var(--bg); font-family: var(--font-sans); outline: none;
}
header { display: flex; gap: 16px; padding: 20px 22px 4px; align-items: flex-start; }
.logo { flex: none; width: 44px; height: auto; }
h1 { font: 600 20px/1.2 var(--font-mono); letter-spacing: -0.01em; margin: 2px 0 4px; }
.expansion {
  font: 500 11px/1.5 var(--font-mono); letter-spacing: .04em;
  color: var(--muted); margin: 0 0 8px;
}
.expansion b { color: var(--accent); font-weight: 600; }
h2 { font-size: 12.5px; font-weight: 600; margin: 0 0 4px; }
h2 small { color: var(--muted); font-weight: 400; font-size: 11px; margin-left: 6px; }
.sub { color: var(--muted); font-size: 12px; line-height: 1.5; margin: 0; }
header .sub + .sub { margin-top: 8px; }
.steps {
  flex: 1; overflow-y: auto; margin: 0; padding: 14px 22px 18px 40px;
  display: flex; flex-direction: column; gap: 18px;
}
.steps li::marker { color: var(--accent); font-weight: 600; font-size: 12px; }
dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 14px; margin: 6px 0; font-size: 12px; }
dt { color: var(--muted); }
dd { margin: 0; font-size: 12px; }
.row { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
.distro { flex: 1 1 130px; min-width: 0; padding: 4px 6px; font-size: 12px; }
.row .btn { padding: 5px 12px; font-size: 12px; }
.row .btn:disabled { opacity: .45; cursor: default; }
.row .btn:disabled:hover { border-color: var(--line); }
.hint { display: block; margin: 6px 0 0; }
footer {
  display: flex; align-items: center; gap: 8px; padding: 12px 22px;
  border-top: 1px solid var(--line);
}
footer .hint { flex: 1; margin: 0; }
</style>
