<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { QUICK_ACTIONS } from "../quickactions";

const previewSrc = ref("");
const message = ref("");
const error = ref("");
const sending = ref(false);

type Session = { sid: string; distro: string; project: string; cwd: string };
const sessions = ref<Session[]>([]);
const selected = ref<string>(""); // "" = clipboard

onMounted(async () => {
  previewSrc.value = convertFileSrc(await invoke<string>("get_capture"));
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") invoke("cancel_capture");
  });
  sessions.value = await invoke<Session[]>("list_sessions_cmd");
  if (sessions.value.length > 0) selected.value = "0"; // auto-select the only/first session
});

async function send() {
  sending.value = true;
  error.value = "";
  try {
    const s = selected.value === "" ? null : sessions.value[Number(selected.value)];
    await invoke("send_capture", {
      message: message.value || null,
      session: s ? { sid: s.sid, distro: s.distro, project: s.project } : null,
    });
  } catch (e) {
    error.value = String(e); // capture is preserved; user can retry (spec §22)
  } finally {
    sending.value = false;
  }
}
</script>

<template>
  <div class="composer">
    <img :src="previewSrc" class="preview" />
    <div class="actions">
      <button v-for="a in QUICK_ACTIONS" :key="a.label" @click="message = a.text">{{ a.label }}</button>
    </div>
    <textarea v-model="message" rows="3" placeholder="Optional message… (default: analyze in current context)" />
    <select v-model="selected" class="target">
      <option v-for="(s, i) in sessions" :key="s.sid" :value="String(i)">
        Claude Code: {{ s.project }} ({{ s.distro }} {{ s.cwd }})
      </option>
      <option value="">Clipboard — paste manually</option>
    </select>
    <p v-if="error" class="error">{{ error }} — Send again to retry.</p>
    <div class="buttons">
      <button @click="invoke('cancel_capture')">Cancel</button>
      <button class="primary" :disabled="sending" @click="send">Send</button>
    </div>
  </div>
</template>

<style scoped>
.composer { display: flex; flex-direction: column; gap: 8px; padding: 10px; height: 100vh; box-sizing: border-box; font-family: system-ui; }
.preview { max-height: 160px; object-fit: contain; border: 1px solid #ccc; }
.actions { display: flex; gap: 6px; flex-wrap: wrap; }
textarea { resize: none; }
.target { font-size: 12px; color: #666; margin: 0; }
.error { font-size: 12px; color: #c00; margin: 0; }
.buttons { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; }
.primary { font-weight: 600; }
</style>
