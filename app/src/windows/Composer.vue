<script setup lang="ts">
import { onMounted, ref, computed, nextTick } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { QUICK_ACTIONS } from "../quickactions";

const captures = ref<string[]>([]);
const current = ref(0); // selected image index
const bust = ref(0); // cache-buster, bumped on every refresh
const previewSrc = computed(() =>
  captures.value.length ? convertFileSrc(captures.value[current.value]) + "?v=" + bust.value : ""
);
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

async function refreshCaptures(selectLast = false) {
  captures.value = await invoke<string[]>("get_captures");
  bust.value++;
  if (selectLast && captures.value.length) current.value = captures.value.length - 1;
  if (current.value >= captures.value.length) current.value = Math.max(0, captures.value.length - 1);
  shapes.value = [];
  annotating.value = false;
}

async function startAnnotate() {
  annotating.value = true;
  await nextTick();
  try {
    img.onload = redraw;
    img.src = await invoke<string>("get_capture_data_url", { index: current.value });
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

async function removeAt(i: number) {
  await invoke("remove_capture", { index: i });
}

onMounted(async () => {
  await refreshCaptures(true);
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") invoke("cancel_capture");
  });
  sessions.value = await invoke<Session[]>("list_sessions_cmd");
  if (sessions.value.length > 0) selected.value = "0"; // auto-select the only/first session
  listen("captures-changed", () => refreshCaptures(true));
  window.addEventListener("paste", async () => {
    error.value = "";
    try {
      await invoke("import_clipboard");
    } catch (e) {
      error.value = String(e);
    }
  });
  getCurrentWebview().onDragDropEvent(async (e) => {
    if (e.payload.type !== "drop") return;
    error.value = "";
    for (const p of e.payload.paths) {
      try {
        await invoke("import_file", { path: p });
      } catch (err) {
        error.value = String(err);
      }
    }
  });
});

async function send() {
  sending.value = true;
  error.value = "";
  try {
    if (shapes.value.length && canvasEl.value) {
      redraw();
      await invoke("save_annotated", { dataUrl: canvasEl.value.toDataURL("image/png"), index: current.value });
    }
    const s = selected.value === "" ? null : sessions.value[Number(selected.value)];
    await invoke("send_capture", {
      message: message.value || null,
      session: s ? { sid: s.sid, distro: s.distro, project: s.project } : null,
    });
  } catch (e) {
    error.value = String(e); // captures are preserved; user can retry (spec §22)
  } finally {
    sending.value = false;
  }
}
</script>

<template>
  <div class="composer">
    <div v-if="!captures.length" class="preview-frame empty-drop">
      <p>Paste an image (Ctrl+V)<br>or drop one here from Explorer</p>
    </div>
    <div v-else-if="!annotating" class="preview-frame">
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
    <div class="rail" v-show="captures.length">
      <div v-for="(c, i) in captures" :key="c" class="thumb" :class="{ sel: i === current }" @click="current = i; shapes = []; annotating = false">
        <img :src="convertFileSrc(c) + '?v=' + bust" draggable="false" />
        <button class="thumb-x" title="Remove from message" @click.stop="removeAt(i)">×</button>
      </div>
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
      <button class="btn btn-primary" :disabled="sending || !captures.length" @click="send">Send</button>
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
.canvas-wrap { display: flex; align-items: center; justify-content: center; }
.canvas { max-width: 100%; max-height: 100%; cursor: crosshair; }
.empty-drop { align-items: center; justify-content: center; border: 1px dashed var(--line); border-radius: 6px; }
.empty-drop p { color: var(--muted); text-align: center; font-size: 12.5px; }
.rail { display: flex; gap: 6px; overflow-x: auto; padding: 2px; }
.thumb { position: relative; flex: none; width: 56px; height: 40px; border: 1px solid var(--line); border-radius: 4px; overflow: hidden; cursor: pointer; }
.thumb.sel { border-color: var(--accent); }
.thumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
.thumb-x {
  position: absolute; top: 1px; right: 1px; width: 15px; height: 15px; line-height: 13px;
  background: rgba(21,23,28,.75); color: #E9EBF0; border: none; border-radius: 3px;
  font-size: 12px; padding: 0; cursor: pointer;
}
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
