import { onMounted, onBeforeUnmount, watch, computed, type Ref, type ComputedRef } from "vue";
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
  onSessionAuthFailed,
  onZmodemRequest,
  onZmodemAskDir,
  onZmodemAskFiles,
  onZmodemConflict,
  onZmodemProgress,
} from "../lib/api";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { registerTerminalActions, unregisterTerminalActions } from "../lib/terminalBus";
import { useSessionStore, type Encoding } from "../stores/session";
import { useSettingsStore, type TerminalColorScheme } from "../stores/settings";
import { useProfilesStore } from "../stores/profiles";
import { useZmodemStore } from "../stores/zmodem";

export interface XtermHandles {
  term: Terminal;
  fit: () => void;
  backgroundColor: ComputedRef<string>;
}

const TERMINAL_THEMES: Record<TerminalColorScheme, import("@xterm/xterm").ITheme> = {
  dark: {
    background: "#1e1e1e",
    foreground: "#d4d4d4",
  },
  light: {
    background: "#f2f2f3",
    foreground: "#333336",
    cursor: "#333336",
    cursorAccent: "#f2f2f3",
    selectionBackground: "#c9c9ce",
  },
  "solarized-dark": {
    background: "#002b36",
    foreground: "#839496",
    cursor: "#839496",
    cursorAccent: "#002b36",
    selectionBackground: "#073642",
  },
  "solarized-light": {
    background: "#fdf6e3",
    foreground: "#657b83",
    cursor: "#657b83",
    cursorAccent: "#fdf6e3",
    selectionBackground: "#eee8d5",
  },
};

function terminalColorScheme(value: string | null | undefined, fallback: TerminalColorScheme): TerminalColorScheme {
  if (
    value === "light" ||
    value === "dark" ||
    value === "solarized-dark" ||
    value === "solarized-light"
  ) return value;
  return fallback;
}

export function useXterm(
  sessionId: string,
  container: Ref<HTMLElement | null>,
  onClosed: () => void,
  onError: (msg: string) => void,
): XtermHandles {
  const sessionStore = useSessionStore();
  const settings = useSettingsStore();
  const profilesStore = useProfilesStore();

  const appearance = computed(() => profilesStore.appearanceForSession(sessionId));

  const terminalTheme = computed<TerminalColorScheme>(() =>
    terminalColorScheme(appearance.value?.color_scheme, settings.colorScheme),
  );
  const backgroundColor = computed(
    () => TERMINAL_THEMES[terminalTheme.value].background ?? "#1e1e1e",
  );

  const term = new Terminal({
    cursorBlink: true,
    fontSize: appearance.value?.font_size ?? settings.fontSize,
    fontFamily: appearance.value?.font_family ?? settings.fontFamily,
    theme: TERMINAL_THEMES[terminalTheme.value ?? settings.theme],
  });
  const fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.loadAddon(new WebLinksAddon());

  let unlistenOutput: (() => void) | null = null;
  let unlistenClosed: (() => void) | null = null;
  let unlistenError: (() => void) | null = null;
  let unlistenAuthFailed: (() => void) | null = null;
  let unlistenZmodemRequest: (() => void) | null = null;
  let unlistenZmodemAskDir: (() => void) | null = null;
  let unlistenZmodemAskFiles: (() => void) | null = null;
  let unlistenZmodemConflict: (() => void) | null = null;
  let unlistenZmodemProgress: (() => void) | null = null;
  let onSelectionChange: { dispose: () => void } | null = null;
  let onMouseDown: ((e: MouseEvent) => void) | null = null;
  let disposed = false;
  let resizeObserver: ResizeObserver | null = null;
  let sizeSyncTimer: number | null = null;
  let sizeSyncDeadline: number | null = null;
  let sizeSynced = false;

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

  /** 把终端真实行列数同步给远端 PTY。新会话的 PTY 初始是 80x24，

   * 而首次 resize 往往在连接建立前就发了（会被后端拒绝），所以必须重试，
   * 否则 vim/nano 等全屏程序只画 24 行 —— 表现为「只显示半屏」。 */
  async function syncSize() {
    if (sizeSynced || disposed) return;
    // 容器不可见时 fit 会得到退化尺寸，跳过，等可见后再同步
    if (term.cols <= 1 || term.rows <= 1) return;
    try {
      await resizeSession(sessionId, term.cols, term.rows);
      sizeSynced = true;
      stopSizeSync();
    } catch {
      /* 会话尚未建立，等下一次重试 */
    }
  }

  function stopSizeSync() {
    if (sizeSyncTimer !== null) {
      window.clearInterval(sizeSyncTimer);
      sizeSyncTimer = null;
    }
    if (sizeSyncDeadline !== null) {
      window.clearTimeout(sizeSyncDeadline);
      sizeSyncDeadline = null;
    }
  }

  function startSizeSync() {
    if (sizeSynced || sizeSyncTimer !== null) return;
    void syncSize();
    sizeSyncTimer = window.setInterval(() => void syncSize(), 400);
    // 兜底：最多重试 20 秒（连接很慢的服务器也不至于一直空转）
    sizeSyncDeadline = window.setTimeout(stopSizeSync, 20000);
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
    startSizeSync();

    // 容器尺寸变化（切标签、折叠侧边栏、最大化、弹窗关闭）时重新 fit，
    // fit 触发 term.onResize，从而把新尺寸同步给远端。
    if (typeof ResizeObserver !== "undefined") {
      let rafId = 0;
      resizeObserver = new ResizeObserver(() => {
        cancelAnimationFrame(rafId);
        rafId = requestAnimationFrame(() => {
          if (!disposed) fit();
        });
      });
      resizeObserver.observe(container.value);
    }

    term.onData((data) => {
      void sendInput(sessionId, data).catch(() => {});
    });
    term.onResize(({ cols, rows }) => {
      void resizeSession(sessionId, cols, rows).catch(() => {});
    });

    // 认证失败且凭据来自已保存档案：弹重输框。
    // 在 replayBuffer 之前尽早订阅，避免与后端失败事件竞速。
    unlistenAuthFailed = await onSessionAuthFailed(sessionId, (p) => {
      if (disposed) return;
      profilesStore.promptAuthRetry(p.profile_id, p.session_id);
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
    registerTerminalActions(sessionId, {
      copy: copySelection,
      paste: pasteFromClipboard,
      focus: () => term.focus(),
    });

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
      // 有输出说明会话已就绪，补发真实尺寸（含重连后的新 PTY）
      if (!sizeSynced) void syncSize();
    });
    if (disposed) {
      unlistenOutput();
      unlistenAuthFailed();
      return;
    }

    unlistenClosed = await onSessionClosed(sessionId, () => {
      if (disposed) return;
      // 重连会新建 PTY（又是 80x24），下次输出时重新同步尺寸
      sizeSynced = false;
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
      unlistenAuthFailed();
      return;
    }

    // ZMODEM：方向路由（ask-dir/ask-files）、三选兜底、冲突裁决、进度状态行
    const zmodemStore = useZmodemStore();
    unlistenZmodemRequest = await onZmodemRequest(sessionId, () => {
      if (disposed) return;
      zmodemStore.request(sessionId);
    });
    // 远端 sz（下载）→ 应用内弹框指定保存目录（点「保存到…」或直接粘贴路径）
    unlistenZmodemAskDir = await onZmodemAskDir(sessionId, () => {
      if (disposed) return;
      zmodemStore.askPick(sessionId, "receive");
    });
    // 远端 rz（上传）→ 应用内弹框选择要发送的文件
    unlistenZmodemAskFiles = await onZmodemAskFiles(sessionId, () => {
      if (disposed) return;
      zmodemStore.askPick(sessionId, "send");
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
    () => [appearance.value, settings.fontFamily, settings.fontSize, settings.colorScheme] as const,
    ([a, globalFamily, globalSize, globalColorScheme]) => {
      if (disposed) return;
      term.options.fontFamily = a?.font_family ?? globalFamily;
      term.options.fontSize = a?.font_size ?? globalSize;
      term.options.theme = TERMINAL_THEMES[terminalColorScheme(a?.color_scheme, globalColorScheme)];
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
    stopSizeSync();
    resizeObserver?.disconnect();
    resizeObserver = null;
    unlistenOutput?.();
    unlistenClosed?.();
    unlistenError?.();
    unlistenAuthFailed?.();
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

  return { term, fit, backgroundColor };
}
