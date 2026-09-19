<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import MenuBar from "./components/MenuBar.vue";
import Toolbar from "./components/Toolbar.vue";
import SessionSidebar from "./components/SessionSidebar.vue";
import TerminalPane from "./components/TerminalPane.vue";
import NewSessionDialog from "./components/NewSessionDialog.vue";
import ContextMenu from "./components/ContextMenu.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import EncodingDialog from "./components/EncodingDialog.vue";
import McpDialog from "./components/McpDialog.vue";
import AboutDialog from "./components/AboutDialog.vue";
import ZmodemDialog from "./components/ZmodemDialog.vue";
import ZmodemConflictDialog from "./components/ZmodemConflictDialog.vue";
import ConnectAuthDialog from "./components/ConnectAuthDialog.vue";
import { useSftpStore } from "./stores/sftp";
import { getTerminalActions } from "./lib/terminalBus";
import { ask } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSessionStore } from "./stores/session";
import { useProfilesStore } from "./stores/profiles";
import { useSessionLog } from "./composables/useSessionLog";
import { onSessionsChanged, onProfilesChanged } from "./lib/api";

const store = useSessionStore();
const sftpStore = useSftpStore();
const sessionLog = useSessionLog();

function onMenuSftp() {
  const id = store.activeId;
  if (id) void sftpStore.openPanelAndConnect(id);
}
const profilesStore = useProfilesStore();
const showDialog = ref(false);
const showEncoding = ref(false);
const showSettings = ref(false);
const showMcp = ref(false);
const showAbout = ref(false);
const dialogOpen = computed({
  get: () => showDialog.value || profilesStore.editingProfile != null,
  set: (v: boolean) => {
    showDialog.value = v;
    if (!v) profilesStore.clearEdit();
  },
});
let unlistenSessions: (() => void) | null = null;
let unlistenProfiles: (() => void) | null = null;

function newSession() {
  showDialog.value = true;
}

function disconnect() {
  if (store.activeId) {
    void store.close(store.activeId);
  }
}

function reconnect() {
  if (store.activeId) {
    void store.reconnect(store.activeId);
  }
}

function menuCopy() {
  const id = store.activeId;
  if (id) getTerminalActions(id)?.copy();
}

function menuPaste() {
  const id = store.activeId;
  if (id) getTerminalActions(id)?.paste();
}

async function quitApp() {
  await new Promise((r) => setTimeout(r, 150));
  const ok = await ask("确定退出 xtshell？未断开的会话将逐个关闭。", {
    title: "退出确认",
    kind: "warning",
    okLabel: "退出",
    cancelLabel: "取消",
  });
  if (!ok) return;
  for (const s of [...store.sessions]) {
    try {
      await store.close(s.id);
    } catch {
      /* 已关闭的跳过，继续 */
    }
  }
  await getCurrentWindow().close();
}

onMounted(async () => {
  await store.refresh();
  unlistenSessions = await onSessionsChanged(() => {
    void store.refresh();
  });
  unlistenProfiles = await onProfilesChanged(() => {
    void profilesStore.refresh();
  });
});

onBeforeUnmount(() => {
  unlistenSessions?.();
  unlistenProfiles?.();
  profilesStore.unbind();
});
</script>

<template>
  <div class="app">
    <MenuBar
      @new-session="newSession"
      @disconnect="disconnect"
      @font-settings="showSettings = true"
      @encoding-settings="showEncoding = true"
      @mcp-info="showMcp = true"
      @sftp="onMenuSftp"
      @copy="menuCopy"
      @paste="menuPaste"
      @reconnect="reconnect"
      @toggle-log="sessionLog.toggleLogging"
      @exit="quitApp"
      @about="showAbout = true"
    />
    <Toolbar
      @new-session="newSession"
      @disconnect="disconnect"
      @reconnect="reconnect"
      @font-settings="showSettings = true"
      @encoding-settings="showEncoding = true"
      @mcp-info="showMcp = true"
    />
    <div class="body">
      <SessionSidebar />
      <div class="terminal-area">
        <TerminalPane @new-session="newSession" />
      </div>
    </div>
    <NewSessionDialog v-model:open="dialogOpen" :edit-profile="profilesStore.editingProfile" />
    <SettingsDialog v-model:open="showSettings" />
    <EncodingDialog v-model:open="showEncoding" />
    <McpDialog v-model:open="showMcp" />
    <AboutDialog v-model:open="showAbout" />
    <ZmodemDialog />
    <ZmodemConflictDialog />
    <ConnectAuthDialog />
    <ContextMenu />
  </div>
</template>

<style>
html,
body,
#app {
  height: 100%;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: #1e1e1e;
}

* {
  box-sizing: border-box;
}
</style>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
}

.body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.terminal-area {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
</style>
