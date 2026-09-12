import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { listSessions, createSession, closeSession, reconnectSession } from "../lib/api";
import type { SessionInfo, SessionConfig } from "../types/session";
import type { Encoding } from "../types/session";
export type { Encoding } from "../types/session";

export const useSessionStore = defineStore("session", () => {
  const sessions = ref<SessionInfo[]>([]);
  const activeId = ref<string | null>(null);
  const sidebarCollapsed = ref(false);
  const encodings = ref<Record<string, Encoding>>({});
  const logging = ref<Record<string, string>>({});

  function encodingOf(id: string): Encoding {
    return encodings.value[id] ?? "utf-8";
  }

  function setEncoding(id: string, enc: Encoding) {
    encodings.value = { ...encodings.value, [id]: enc };
  }

  function setLogging(id: string, path: string | null) {
    const next = { ...logging.value };
    if (path) next[id] = path;
    else delete next[id];
    logging.value = next;
  }
  const activeSession = computed(
    () => sessions.value.find((s) => s.id === activeId.value) ?? null,
  );

  async function refresh() {
    const prev = sessions.value;
    const next = await listSessions();
    sessions.value = next;

    // 新出现的会话（含 MCP/外部创建）自动激活，tab 随之可见
    const added = next.filter((s) => !prev.some((p) => p.id === s.id));
    if (added.length > 0) {
      activeId.value = added[added.length - 1].id;
      return;
    }

    // 当前激活会话已被外部关闭时，回退到剩余会话
    if (activeId.value && !next.some((s) => s.id === activeId.value)) {
      activeId.value = next[next.length - 1]?.id ?? null;
    }
  }

  async function create(config: SessionConfig): Promise<string> {
    const id = await createSession(config);
    await refresh();
    activeId.value = id;
    return id;
  }

  async function close(id: string) {
    await closeSession(id);
    if (activeId.value === id) {
      activeId.value = sessions.value.find((s) => s.id !== id)?.id ?? null;
    }
    await refresh();
  }

  async function reconnect(id: string): Promise<string | null> {
    try {
      const newId = await reconnectSession(id);
      await refresh();
      return newId;
    } catch {
      return null;
    }
  }

  function setActive(id: string | null) {
    activeId.value = id;
  }

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value;
  }

  // 侧栏面板切换（工具栏 SFTP 按钮与侧栏页签共享）
  const sftpPanelActive = ref(false);

  return {
    sessions,
    activeId,
    sidebarCollapsed,
    sftpPanelActive,
    encodings,
    encodingOf,
    setEncoding,
    logging,
    setLogging,
    activeSession,
    refresh,
    create,
    close,
    reconnect,
    setActive,
    toggleSidebar,
  };
});
