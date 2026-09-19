import { onMounted, onBeforeUnmount, watch, type Ref } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import {
  sendInput,
  resizeSession,
  getSessionOutput,
  onSessionOutput,
  onSessionClosed,
  onSessionError,
  onZmodemRequest,
  onZmodemAskDir,
  onZmodemAskFiles,
  onZmodemConflict,
  onZmodemProgress,
  zmodemStartReceive,
  zmodemSendFiles,
  zmodemCancel,
} from "../lib/api";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import { registerTerminalActions, unregisterTerminalActions } from "../lib/terminalBus";
import { useSessionStore, type Encoding } from "../stores/session";
import { useSettingsStore } from "../stores/settings";
import { useZmodemStore } from "../stores/zmodem";

export interface XtermHandles {
  term: Terminal;
  fit: () => void;
}

export function useXterm(
  sessionId: string,
  container: Ref<HTMLElement | null>,
  onClosed: () => void,
  onError: (msg: string) => void,
): XtermHandles {
  const sessionStore = useSessionStore();
  const settings = useSettingsStore();

  const term = new Terminal({
    cursorBlink: true,
    fontSize: settings.fontSize,
    fontFamily: settings.fontFamily,
    theme: {
      background: "#1e1e1e",
      foreground: "#d4d4d4",
    },
  });
  const fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.loadAddon(new WebLinksAddon());

  let unlistenOutput: (() => void) | null = null;
  let unlistenClosed: (() => void) | null = null;
  let unlistenError: (() => void) | null = null;
  let unlistenZmodemRequest: (() => void) | null = null;
  let unlistenZmodemAskDir: (() => void) | null = null;
  let unlistenZmodemAskFiles: (() => void) | null = null;
  let unlistenZmodemConflict: (() => void) | null = null;
  let unlistenZmodemProgress: (() => void) | null = null;
  let onSelectionChange: { dispose: () => void } | null = null;
  let onMouseDown: ((e: MouseEvent) => void) | null = null;
  let disposed = false;

  let decoder = new TextDecoder(sessionStore.encodingOf(sessionId));
  function applyEncoding(enc: Encoding) {
    try {
      decoder = new TextDecoder(enc);
    } catch {
      decoder = new TextDecoder();
    }
  }

  function fit() {
    try {
      fitAddon.fit();
    } catch {
      /* container hidden or zero-sized */
    }
  }

  async function replayBuffer() {
    try {
      const [bytes] = await getSessionOutput(sessionId, 0);
      if (disposed) return;
      if (bytes.length > 0) {
        term.write(decoder.decode(new Uint8Array(bytes), { stream: false }));
      }
    } catch {
      /* 会话可能已被移除 */
    }
  }

  onMounted(async () => {
    if (!container.value) return;
    term.open(container.value);
    fit();
    term.focus();
    resizeSession(sessionId, term.cols, term.rows).catch(() => {});

    term.onData((data) => {
      void sendInput(sessionId, data).catch(() => {});
    });
    term.onResize(({ cols, rows }) => {
      void resizeSession(sessionId, cols, rows).catch(() => {});
    });

    // 复制/粘贴动作（供按钮栏与右键复用）
    async function pasteFromClipboard() {
      try {
        const text = await readText();
        if (text && !disposed) term.paste(text);
      } catch {
        /* 剪贴板为空或不可读 */
      }
    }
    function copySelection() {
      const text = term.getSelection();
      if (text && !disposed) {
        void writeText(text).catch(() => {});
      }
    }
    registerTerminalActions(sessionId, { copy: copySelection, paste: pasteFromClipboard });

    onSelectionChange = term.onSelectionChange(copySelection);

    // 右键即粘贴
    const el = container.value;
    onMouseDown = (e: MouseEvent) => {
      if (e.button !== 2 || disposed) return;
      e.preventDefault();
      void pasteFromClipboard();
    };
    el.addEventListener("mousedown", onMouseDown);
    el.addEventListener("contextmenu", (e) => e.preventDefault());

    await replayBuffer();
    if (disposed) return;

    unlistenOutput = await onSessionOutput(sessionId, (data) => {
      if (disposed) return;
      term.write(decoder.decode(data, { stream: true }));
    });
    if (disposed) {
      unlistenOutput();
      return;
    }

    unlistenClosed = await onSessionClosed(sessionId, () => {
      if (disposed) return;
      term.write("\r\n\x1b[90m[连接已断开]\x1b[0m\r\n");
      onClosed();
    });
    if (disposed) {
      unlistenOutput();
      unlistenClosed();
      return;
    }

    unlistenError = await onSessionError(sessionId, (msg) => {
      if (disposed) return;
      term.write(`\r\n\x1b[31m${msg}\x1b[0m\r\n`);
      onError(msg);
    });
    if (disposed) {
      unlistenOutput();
      unlistenClosed();
      unlistenError();
      return;
    }

    // ZMODEM：方向路由（ask-dir/ask-files）、三选兜底、冲突裁决、进度状态行
    const zmodemStore = useZmodemStore();
    unlistenZmodemRequest = await onZmodemRequest(sessionId, () => {
      if (disposed) return;
      zmodemStore.request(sessionId);
    });
    // 远端 sz（下载）→ 目录选择框 → 一次性接收整批
    let askDirBusy = false;
    unlistenZmodemAskDir = await onZmodemAskDir(sessionId, () => {
      if (disposed || askDirBusy) return;
      askDirBusy = true;
      void (async () => {
        try {
          // 延迟弹原生对话框：避开事件回调内同步弹窗导致的鼠标输入阻塞
          await new Promise((r) => setTimeout(r, 150));
          const dir = await open({ directory: true });
          if (dir) {
            await zmodemStartReceive(sessionId, dir as string);
          } else {
            await zmodemCancel(sessionId);
            term.write("\r\x1b[33m[ZMODEM] 已取消接收\x1b[0m\r\n");
          }
        } catch {
          void zmodemCancel(sessionId).catch(() => {});
        } finally {
          askDirBusy = false;
        }
      })();
    });
    // 远端 rz（上传）→ 文件选择框（可多选）
    let askFilesBusy = false;
    unlistenZmodemAskFiles = await onZmodemAskFiles(sessionId, () => {
      if (disposed || askFilesBusy) return;
      askFilesBusy = true;
      void (async () => {
        try {
          await new Promise((r) => setTimeout(r, 150));
          const paths = await open({ multiple: true });
          if (paths) {
            const list = Array.isArray(paths) ? paths : [paths];
            await zmodemSendFiles(sessionId, list);
          } else {
            await zmodemCancel(sessionId);
            term.write("\r\x1b[33m[ZMODEM] 已取消发送，远端已中止\x1b[0m\r\n");
          }
        } catch {
          void zmodemCancel(sessionId).catch(() => {});
        } finally {
          askFilesBusy = false;
        }
      })();
    });
    // 同名冲突裁决 → 队列化弹自定义对话框
    unlistenZmodemConflict = await onZmodemConflict(sessionId, (info) => {
      if (disposed) return;
      zmodemStore.addConflict(sessionId, info);
    });
    unlistenZmodemProgress = await onZmodemProgress(sessionId, (p: import("../lib/api").ZmodemProgress) => {
      if (disposed) return;
      const pct = p.total > 0 ? Math.round((p.done / p.total) * 100) : 0;
      let line = "";
      switch (p.kind) {
        case "start":
          line = p.name
            ? `\x1b[36m[ZMODEM] ${p.name}\x1b[0m`
            : "\x1b[36m[ZMODEM] 传输开始\x1b[0m";
          break;
        case "progress":
          line = `\x1b[36m[ZMODEM] ${p.name} ${pct}%\x1b[0m`;
          break;
        case "saved":
          line = `\x1b[32m[ZMODEM] 已保存 ${p.name}\x1b[0m\r\n`;
          break;
        case "unsaved":
          line = `\x1b[33m[ZMODEM] ${p.name} 已存于临时目录（未选保存路径）\x1b[0m\r\n`;
          break;
        case "complete":
          line = "\x1b[32m[ZMODEM] 会话完成\x1b[0m\r\n";
          break;
        case "cancelled":
          line = "\x1b[33m[ZMODEM] 已取消\x1b[0m\r\n";
          break;
        case "aborted":
          line = "\x1b[31m[ZMODEM] 传输中止\x1b[0m\r\n";
          break;
        case "error":
          line = `\x1b[31m[ZMODEM] ${p.name}\x1b[0m\r\n`;
          break;
      }
      if (line) term.write(`\r\x1b[K${line}`);
    });
    if (disposed) {
      unlistenOutput();
      unlistenClosed();
      unlistenError();
      unlistenZmodemRequest();
      unlistenZmodemAskDir();
      unlistenZmodemAskFiles();
      unlistenZmodemConflict();
      unlistenZmodemProgress();
      return;
    }

    window.addEventListener("resize", handleWindowResize);
  });

  watch(
    () => sessionStore.encodings[sessionId],
    async (enc) => {
      if (disposed || !enc) return;
      applyEncoding(enc);
      term.reset();
      await replayBuffer();
    },
  );

  watch(
    () => [settings.fontFamily, settings.fontSize] as const,
    ([family, size]) => {
      if (disposed) return;
      term.options.fontFamily = family;
      term.options.fontSize = size;
      fit();
    },
  );

  function handleWindowResize() {
    fit();
  }

  onBeforeUnmount(() => {
    if (disposed) return;
    disposed = true;
    window.removeEventListener("resize", handleWindowResize);
    unlistenOutput?.();
    unlistenClosed?.();
    unlistenError?.();
    unlistenZmodemRequest?.();
    unlistenZmodemAskDir?.();
    unlistenZmodemAskFiles?.();
    unlistenZmodemConflict?.();
    unlistenZmodemProgress?.();
    onSelectionChange?.dispose();
    unregisterTerminalActions(sessionId);
    container.value?.removeEventListener("mousedown", onMouseDown!);
    term.dispose();
  });

  return { term, fit };
}
