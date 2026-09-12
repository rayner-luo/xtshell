import { defineStore } from "pinia";
import { reactive, ref } from "vue";
import {
  sftpConnect,
  sftpDisconnect,
  sftpList,
  sftpMkdir,
  sftpRemove,
  sftpRename,
  sftpCheckUpload,
  sftpUpload,
  sftpDownload,
  onSftpProgress,
  type SftpEntry,
  type SftpProgress,
} from "../lib/api";
import { ask, save } from "@tauri-apps/plugin-dialog";

interface SftpSessionState {
  connected: boolean;
  cwd: string;
  entries: SftpEntry[];
  loading: boolean;
  error: string | null;
  progress: SftpProgress | null;
}

export const useSftpStore = defineStore("sftp", () => {
  const sessions = reactive<Record<string, SftpSessionState>>({});
  const progressListeners = new Map<string, Promise<() => void>>();
  const activeId = ref<string | null>(null);

  function bindProgress(id: string) {
    if (progressListeners.has(id)) return;
    const p = onSftpProgress(id, (pr) => {
      const st = stateOf(id);
      if (pr.kind === "done") {
        setTimeout(() => {
          if (sessions[id]) sessions[id].progress = null;
        }, 800);
      }
      st.progress = pr;
    });
    progressListeners.set(id, p);
  }

  /** 打开 SFTP 面板并确保连接（工具栏按钮与菜单共用入口） */
  async function openPanelAndConnect(id: string) {
    const { useSessionStore } = await import("./session");
    useSessionStore().sftpPanelActive = true;
    const st = stateOf(id);
    if (!st.connected && !st.loading) {
      await connect(id);
    }
  }

  function stateOf(id: string): SftpSessionState {
    if (!sessions[id]) {
      sessions[id] = {
        connected: false,
        cwd: "/",
        entries: [],
        loading: false,
        error: null,
        progress: null,
      };
    }
    return sessions[id];
  }

  function setActive(id: string | null) {
    activeId.value = id;
  }

  async function connect(id: string, path?: string) {
    const st = stateOf(id);
    st.loading = true;
    st.error = null;
    try {
      const [cwd, entries] = await sftpConnect(id, path);
      st.connected = true;
      st.cwd = cwd;
      st.entries = entries;
    } catch (e) {
      st.error = e instanceof Error ? e.message : String(e);
    } finally {
      st.loading = false;
    }
  }

  async function disconnect(id: string) {
    await sftpDisconnect(id);
    const st = stateOf(id);
    st.connected = false;
    st.entries = [];
    st.progress = null;
  }

  async function list(id: string, path: string) {
    const st = stateOf(id);
    st.loading = true;
    st.error = null;
    try {
      st.entries = await sftpList(id, path);
      st.cwd = path;
    } catch (e) {
      st.error = e instanceof Error ? e.message : String(e);
    } finally {
      st.loading = false;
    }
  }

  async function refresh(id: string) {
    const st = stateOf(id);
    await list(id, st.cwd);
  }

  async function mkdir(id: string, name: string) {
    const st = stateOf(id);
    const path = `${st.cwd.replace(/\/$/, "")}/${name}`;
    await sftpMkdir(id, path);
    await refresh(id);
  }

  /** 删除：统一确认弹窗（文件/目录都确认一次） */
  async function remove(id: string, entry: SftpEntry) {
    await new Promise((r) => setTimeout(r, 150));
    const ok = await ask(`确认删除${entry.is_dir ? "目录（含全部内容）" : "文件"}「${entry.name}」？`, {
      title: "确认删除",
      kind: "warning",
      okLabel: "删除",
      cancelLabel: "取消",
    });
    if (!ok) return;
    const st = stateOf(id);
    const path = `${st.cwd.replace(/\/$/, "")}/${entry.name}`;
    try {
      await sftpRemove(id, path, entry.is_dir);
      await refresh(id);
    } catch (e) {
      st.error = e instanceof Error ? e.message : String(e);
    }
  }

  async function renameEntry(id: string, entry: SftpEntry, newName: string) {
    const st = stateOf(id);
    const old = `${st.cwd.replace(/\/$/, "")}/${entry.name}`;
    const now = `${st.cwd.replace(/\/$/, "")}/${newName}`;
    try {
      await sftpRename(id, old, now);
      await refresh(id);
    } catch (e) {
      st.error = e instanceof Error ? e.message : String(e);
    }
  }

  /** 下载：每次弹「另存为」，默认远端文件名 */
  async function download(id: string, entry: SftpEntry) {
    await new Promise((r) => setTimeout(r, 150));
    const local = await save({ defaultPath: entry.name });
    if (!local) return;
    const st = stateOf(id);
    const remote = `${st.cwd.replace(/\/$/, "")}/${entry.name}`;
    try {
      await sftpDownload(id, remote, local);
    } catch (e) {
      st.error = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * 上传：多选本地文件 → 冲突预检 → 首个冲突弹一次询问（覆盖/跳过）→ 批量上传
   * 返回 (成功数, 跳过数)
   */
  async function upload(id: string, localPaths: string[]): Promise<[number, number] | null> {
    const st = stateOf(id);
    try {
      const names = localPaths.map((p) =>
        p.replace(/\\/g, "/").split("/").pop() ?? "file",
      );
      const conflicts = await sftpCheckUpload(id, st.cwd, names);
      let policy = "overwrite";
      if (conflicts.length > 0) {
        await new Promise((r) => setTimeout(r, 150));
        const overwrite = await ask(
          `${conflicts.length} 个同名文件已存在于远端目录：\n${conflicts.join("、")}\n\n覆盖则全部替换，跳过则保留远端原文件（应用于本批）`,
          {
            title: "文件已存在",
            kind: "warning",
            okLabel: "覆盖",
            cancelLabel: "跳过",
          },
        );
        policy = overwrite ? "overwrite" : "skip";
      }
      const [done, skipped] = await sftpUpload(id, st.cwd, localPaths, policy);
      await refresh(id);
      return [done, skipped];
    } catch (e) {
      st.error = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  return {
    sessions,
    activeId,
    setActive,
    stateOf,
    connect,
    disconnect,
    list,
    refresh,
    mkdir,
    remove,
    renameEntry,
    download,
    upload,
    bindProgress,
    openPanelAndConnect,
  };
});
