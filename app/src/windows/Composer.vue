<script setup lang="ts">
import { onMounted, ref, nextTick } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { QUICK_ACTIONS } from "../quickactions";

const previewSrc = ref("");
const message = ref("");
const error = ref("");
const sending = ref(false);

type Session = { sid: string; distro: string; project: string; cwd: string };
const sessions = ref<Session[]>([]);
const selected = ref<string>(""); // "" = clipboard

// annotation state
// ponytail: pen/arrow/rect in fixed red only; circle/text/blur when someone misses them
type Tool = "pen" | "arrow" | "rect";
type Shape = { tool: Tool; points: { x: number; y: number }[] };
const annotating = ref(false);
const tool = ref<Tool>("pen");
const shapes = ref<Shape[]>([]);
const canvasEl = ref<HTMLCanvasElement | null>(null);
const img = new Image();
let drawing = false;

async function startAnnotate() {
  annotating.value = true;
  await nextTick();
  try {
    img.onload = redraw;
    img.src = await invoke<string>("get_capture_data_url");
  } catch (e) {
    error.value = String(e);
    annotating.value = false;
  }
}

function canvasPoint(e: MouseEvent) {
  const c = canvasEl.value!;
  const r = c.getBoundingClientRect();
  return { x: ((e.clientX - r.left) / r.width) * c.width, y: ((e.clientY - r.top) / r.height) * c.height };
}

function down(e: MouseEvent) {
  drawing = true;
  shapes.value.push({ tool: tool.value, points: [canvasPoint(e)] });
}
function move(e: MouseEvent) {
  if (!drawing) return;
  const s = shapes.value[shapes.value.length - 1];
  const p = canvasPoint(e);
  if (s.tool === "pen") s.points.push(p);
  else s.points[1] = p; // arrow/rect: anchor + current
  redraw();
}
function up() {
  drawing = false;
}
function undo() {
  shapes.value.pop();
  redraw();
}

function drawShape(ctx: CanvasRenderingContext2D, s: Shape, scale: number) {
  ctx.strokeStyle = "#e11";
  ctx.lineWidth = 3 * scale;
  ctx.lineCap = "round";
  const pts = s.points;
  if (s.tool === "pen") {
    ctx.beginPath();
    pts.forEach((p, i) => (i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)));
    ctx.stroke();
  } else if (pts.length === 2) {
    const [a, b] = pts;
    if (s.tool === "rect") {
      ctx.strokeRect(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.abs(b.x - a.x), Math.abs(b.y - a.y));
    } else {
      // arrow: shaft + head
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      const ang = Math.atan2(b.y - a.y, b.x - a.x);
      const head = 12 * scale;
      ctx.lineTo(b.x - head * Math.cos(ang - 0.5), b.y - head * Math.sin(ang - 0.5));
      ctx.moveTo(b.x, b.y);
      ctx.lineTo(b.x - head * Math.cos(ang + 0.5), b.y - head * Math.sin(ang + 0.5));
      ctx.stroke();
    }
  }
}

function redraw() {
  const c = canvasEl.value;
  if (!c || !img.naturalWidth) return;
  c.width = img.naturalWidth;
  c.height = img.naturalHeight;
  const ctx = c.getContext("2d")!;
  ctx.drawImage(img, 0, 0);
  shapes.value.forEach((s) => drawShape(ctx, s, Math.max(1, c.width / 800)));
}

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
    if (shapes.value.length && canvasEl.value) {
      redraw();
      await invoke("save_annotated", { dataUrl: canvasEl.value.toDataURL("image/png") });
    }
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
    <div v-if="!annotating" class="preview-frame">
      <img :src="previewSrc" class="preview" @click="startAnnotate" title="Click to annotate" />
    </div>
    <div v-else class="canvas-wrap preview-frame">
      <canvas
        ref="canvasEl"
        class="canvas"
        @mousedown="down"
        @mousemove="move"
        @mouseup="up"
        @mouseleave="up"
      />
    </div>
    <div v-if="annotating" class="tools">
      <button :class="{ on: tool === 'pen' }" @click="tool = 'pen'">Pen</button>
      <button :class="{ on: tool === 'arrow' }" @click="tool = 'arrow'">Arrow</button>
      <button :class="{ on: tool === 'rect' }" @click="tool = 'rect'">Rect</button>
      <button @click="undo" :disabled="!shapes.length">Undo</button>
    </div>
    <div class="actions">
      <button v-for="a in QUICK_ACTIONS" :key="a.label" @click="message = a.text">{{ a.label }}</button>
    </div>
    <textarea v-model="message" rows="3" class="textarea" placeholder="Optional message… (default: analyze in current context)" />
    <select v-model="selected" class="selectbox">
      <option v-for="(s, i) in sessions" :key="s.sid" :value="String(i)">
        Claude Code: {{ s.project }} ({{ s.distro }} {{ s.cwd }})
      </option>
      <option value="">Clipboard — paste manually</option>
    </select>
    <p v-if="error" class="error-text">{{ error }} — Send again to retry.</p>
    <div class="buttons">
      <button class="btn btn-quiet" @click="invoke('cancel_capture')">Cancel</button>
      <button class="btn btn-primary" :disabled="sending" @click="send">Send</button>
    </div>
  </div>
</template>

<style scoped>
.composer { display: flex; flex-direction: column; gap: 10px; padding: 14px; height: 100vh; box-sizing: border-box; background: var(--bg); font-family: var(--font-sans); }
.preview-frame { position: relative; flex: 1; min-height: 160px; display: flex; }
.preview-frame::before, .preview-frame::after {
  content: ""; position: absolute; width: 14px; height: 14px; border: 2.5px solid var(--accent); pointer-events: none; z-index: 1;
}
.preview-frame::before { top: -2px; left: -2px; border-right: none; border-bottom: none; }
.preview-frame::after { bottom: -2px; right: -2px; border-left: none; border-top: none; }
.preview { width: 100%; height: 100%; object-fit: contain; border: 1px solid var(--line); cursor: pointer; }
.canvas-wrap { display: flex; align-items: center; justify-content: center; overflow: hidden; }
.canvas { max-width: 100%; max-height: 100%; cursor: crosshair; }
.tools { display: inline-flex; background: var(--raised); border: 1px solid var(--line); border-radius: 8px; padding: 3px; gap: 2px; }
.tools button { background: transparent; border: none; border-radius: 6px; padding: 4px 10px; color: var(--muted); cursor: pointer; font: 500 12px var(--font-sans); }
.tools button.on { background: var(--bg); color: var(--accent); }
.tools button:disabled { opacity: .45; cursor: default; }
.actions { display: flex; gap: 6px; flex-wrap: wrap; }
.actions button {
  background: transparent; color: var(--muted); border: 1px solid var(--line); border-radius: 999px;
  font: 500 12px var(--font-sans); padding: 4px 11px; cursor: pointer;
}
.actions button:hover { color: var(--text); border-color: var(--muted); }
.selectbox { width: 100%; font-family: var(--font-sans); }
.buttons { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; }
</style>
