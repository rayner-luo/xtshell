<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "../stores/session";
import { useSftpStore } from "../stores/sftp";
import { openCtxMenu } from "../composables/contextMenu";
import type { SftpEntry } from "../lib/api";

const sessionStore = useSessionStore();
const sftpStore = useSftpStore();

const activeId = computed(() => sessionStore.activeId);
const st = computed(() =>
  activeId.value ? sftpStore.stateOf(activeId.value) : null,
);
const isSsh = computed(() => sessionStore.activeSession?.protocol === "ssh");

// 活动会话变化：注册进度监听
watch(
  activeId,
  (id) => {
    if (id) void sftpStore.bindProgress(id);
  },
  { immediate: true },
);

// 内联输入对话框（新建目录/重命名共用）
interface InputDialog {
  title: string;
  message: string;
  value: string;
  resolve: (v: string | null) => void;
}
const inputDialog = ref<InputDialog | null>(null);
const inputEl = ref<HTMLInputElement | null>(null);

function askName(title: string, message: string, defaultValue = ""): Promise<string | null> {
  return new Promise((resolve) => {
    inputDialog.value = { title, message, value: defaultValue, resolve };
    setTimeout(() => inputEl.value?.focus(), 30);
  });
}

function commitInput() {
  const d = inputDialog.value;
  const v = d?.value.trim() || null;
  d?.resolve(v);
  inputDialog.value = null;
}

function cancelInput() {
  inputDialog.value?.resolve(null);
  inputDialog.value = null;
}

function fmtSize(e: SftpEntry): string {
  if (e.is_dir) return "";
  if (e.size < 1024) return `${e.size} B`;
  if (e.size < 1024 * 1024) return `${(e.size / 1024).toFixed(1)}K`;
  return `${(e.size / 1024 / 1024).toFixed(1)}M`;
}

async function disconnect() {
  if (!activeId.value) return;
  await sftpStore.disconnect(activeId.value);
}

async function refresh() {
  if (!activeId.value) return;
  await sftpStore.refresh(activeId.value);
}

function gotoParent() {
  const id = activeId.value;
  if (!id || !st.value) return;
  const parent = st.value.cwd
    .replace(/\/+$/, "")
    .split("/")
    .slice(0, -1)
    .join("/");
  void sftpStore.list(id, parent || "/");
}

function openEntry(e: SftpEntry) {
  const id = activeId.value;
  if (!id || !st.value) return;
  if (e.is_dir) {
    void sftpStore.list(id, `${st.value.cwd.replace(/\/$/, "")}/${e.name}`);
  }
}

async function newDir() {
  const id = activeId.value;
  if (!id) return;
  const name = await askName("新建目录", "目录名：");
  if (name) await sftpStore.mkdir(id, name);
}

async function renameEntry(e: SftpEntry) {
  const id = activeId.value;
  if (!id) return;
  const name = await askName("重命名", "新名称：", e.name);
  if (name && name !== e.name) await sftpStore.renameEntry(id, e, name);
}

async function uploadFiles() {
  const id = activeId.value;
  if (!id) return;
  await new Promise((r) => setTimeout(r, 150));
  try {
    const paths = await open({ multiple: true });
    if (paths) {
      const list = Array.isArray(paths) ? paths : [paths];
      await sftpStore.upload(id, list);
    }
  } catch {
    /* ignore */
  }
}

/** 文件名列右键：下载（文件）/ 重命名 / 删除 */
function entryNameMenu(e: MouseEvent, entry: SftpEntry) {
  const id = activeId.value;
  if (!id) return;
  openCtxMenu(e, [
    ...(entry.is_dir
      ? []
      : [{ label: "下载", action: () => void sftpStore.download(id, entry) }]),
    { label: "重命名", action: () => void renameEntry(entry) },
    { label: "删除", action: () => void sftpStore.remove(id, entry) },
  ]);
}

/** 大小列右键：目录级操作 + 断开 */
function dirMenu(e: MouseEvent) {
  openCtxMenu(e, [
    { label: "上传文件…", action: () => void uploadFiles() },
    { label: "新建目录", action: () => void newDir() },
    { label: "刷新", action: () => void refresh() },
    { label: "断开 SFTP", action: () => void disconnect() },
  ]);
}

function panelMenu(e: MouseEvent) {
  dirMenu(e);
}

function progressPct(): number {
  const p = st.value?.progress;
  if (!p || p.total === 0) return 0;
  return Math.round((p.done / p.total) * 100);
}
</script>

<template>
  <div class="sftp-panel">
    <template v-if="isSsh && activeId">
      <template v-if="st?.connected">
        <div class="sftp-path">
          <!-- \u200E (LRM) 强制整条路径按 LTR 排布，否则 RTL 段落会把开头的 / 挪到行尾 -->
          <span class="path-text" :title="st.cwd">{{ "\u200E" + st.cwd }}</span>
        </div>
        <div class="sftp-list" @contextmenu.prevent="panelMenu">
          <div v-if="st.loading && st.entries.length === 0" class="sftp-hint">加载中…</div>
          <div v-if="st.cwd !== '/'" class="sftp-entry parent-entry" @dblclick="gotoParent">
            <span class="entry-icon">📁</span>
            <span class="entry-name">..</span>
            <span class="entry-size"></span>
          </div>
          <div
            v-for="e in st.entries"
            :key="e.name"
            class="sftp-entry"
            @dblclick="openEntry(e)"
          >
            <span
              class="entry-name"
              :title="e.name"
              @contextmenu.prevent.stop="entryNameMenu($event, e)"
            >
              {{ e.is_dir ? "📁" : "📄" }} {{ e.name }}
            </span>
            <span
              class="entry-size"
              @contextmenu.prevent.stop="dirMenu($event)"
            >
              {{ fmtSize(e) }}
            </span>
          </div>
          <div v-if="!st.loading && st.entries.length === 0" class="sftp-hint">空目录</div>
        </div>
        <div v-if="st.progress" class="sftp-progress">
          <template v-if="st.progress.kind === 'progress'">
            <span class="p-name">{{ st.progress.name }}</span>
            <span class="p-pct">{{ progressPct() }}%</span>
          </template>
          <span v-else class="p-name">{{ st.progress.kind === "skip" ? "跳过" : "" }} {{ st.progress.name }}</span>
        </div>
        <div v-if="st.error" class="sftp-error">{{ st.error }}</div>
      </template>
      <div v-else class="sftp-hint grow">
        {{ st?.loading ? "连接中…" : "点击工具栏「SFTP」按钮连接" }}
      </div>
    </template>
    <div v-else class="sftp-hint grow">当前会话不支持 SFTP（仅 SSH）</div>

    <!-- 输入对话框：新建目录/重命名 -->
    <Teleport to="body">
      <div v-if="inputDialog" class="dlg-overlay">
        <div class="dlg">
          <div class="dlg-title">{{ inputDialog.title }}</div>
          <div class="dlg-msg">{{ inputDialog.message }}</div>
          <input
            ref="inputEl"
            v-model="inputDialog.value"
            class="dlg-input"
            @keyup.enter="commitInput"
            @keyup.esc="cancelInput"
          />
          <div class="dlg-footer">
            <button class="sbtn" @click="cancelInput">取消</button>
            <button class="sbtn primary" @click="commitInput">确定</button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.sftp-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.sbtn {
  padding: 3px 8px;
  font-size: 11px;
  color: #cccccc;
  background: #2d2d2d;
  border: 1px solid #444444;
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
}

.sbtn:hover:not(:disabled) {
  background: #3c3c3c;
  color: #ffffff;
}

.sbtn:disabled {
  opacity: 0.5;
  cursor: default;
}

.sbtn.primary {
  color: #1e1e1e;
  background: #4ec9b0;
  border-color: #4ec9b0;
  font-weight: 600;
}

.sftp-path {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 8px 6px;
  flex-shrink: 0;
}

.path-text {
  flex: 1;
  min-width: 0;
  font-size: 11px;
  color: #888888;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  direction: rtl;
  text-align: left;
}

.sftp-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  border-top: 1px solid #2e2e2e;
  scrollbar-width: thin;
  scrollbar-color: rgba(121, 121, 121, 0.4) transparent;
}

/* VS Code 风格细滚动条（与终端 xterm 滚动条一致） */
.sftp-list::-webkit-scrollbar {
  width: 10px;
}

.sftp-list::-webkit-scrollbar-track {
  background: transparent;
}

.sftp-list::-webkit-scrollbar-thumb {
  background: rgba(121, 121, 121, 0.4);
  border-radius: 5px;
}

.sftp-list::-webkit-scrollbar-thumb:hover {
  background: rgba(100, 100, 100, 0.7);
}

.sftp-entry {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 10px;
  font-size: 12px;
  color: #cccccc;
  cursor: pointer;
  user-select: none;
}

.sftp-entry:hover {
  background: #2f2f2f;
}

.parent-entry .entry-name {
  color: #999999;
}

.entry-name {
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.entry-size {
  flex: 0 0 54px;
  font-size: 10px;
  color: #777777;
  text-align: right;
}

.sftp-progress {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px;
  font-size: 11px;
  color: #4ec9b0;
  border-top: 1px solid #2e2e2e;
  flex-shrink: 0;
}

.p-name {
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.p-pct {
  flex-shrink: 0;
}

.sftp-hint {
  padding: 16px 10px;
  font-size: 12px;
  color: #777777;
  text-align: center;
}

.sftp-hint.grow {
  flex: 1;
}

.sftp-error {
  padding: 4px 10px;
  font-size: 11px;
  color: #e06c75;
  flex-shrink: 0;
}

.dlg-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dlg {
  width: 300px;
  background: #252525;
  border: 1px solid #3c3c3c;
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  padding: 14px 16px;
}

.dlg-title {
  font-size: 13px;
  font-weight: 600;
  color: #ffffff;
  margin-bottom: 6px;
}

.dlg-msg {
  font-size: 11px;
  color: #999999;
  margin-bottom: 10px;
}

.dlg-input {
  width: 100%;
  padding: 6px 10px;
  font-size: 12px;
  color: #e0e0e0;
  background: #1a1a1a;
  border: 1px solid #444444;
  border-radius: 4px;
  outline: none;
  box-sizing: border-box;
}

.dlg-input:focus {
  border-color: #4ec9b0;
}

.dlg-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
}
</style>
