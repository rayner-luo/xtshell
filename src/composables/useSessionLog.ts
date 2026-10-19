import { computed } from "vue";
import { useSessionStore } from "../stores/session";
import { setSessionLog } from "../lib/api";
import { save } from "@tauri-apps/plugin-dialog";

/** 默认日志文件名：年月日时分秒.txt（例如 20261019024713.txt） */
function defaultLogName(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return (
    `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}` +
    `${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}.txt`
  );
}

/** 会话日志开关：工具栏日志按钮与会话菜单共用 */
export function useSessionLog() {
  const store = useSessionStore();

  const logPath = computed(() => {
    const id = store.activeId;
    return id != null ? store.logging[id] ?? null : null;
  });
  const isRecording = computed(() => !!logPath.value);
  const logTooltip = computed(() =>
    isRecording.value ? `停止记录\n${logPath.value}` : "记录会话日志",
  );

  async function toggleLogging() {
    const id = store.activeId;
    if (!id) return;
    try {
      if (logPath.value) {
        await setSessionLog(id, null);
        store.setLogging(id, null);
      } else {
        // 「另存为」：目录与文件名都由用户指定，默认用时间戳命名
        const path = await save({
          title: "保存会话日志",
          defaultPath: defaultLogName(),
          filters: [{ name: "文本日志", extensions: ["txt", "log"] }],
        });
        if (!path) return;
        const saved = await setSessionLog(id, path);
        store.setLogging(id, saved ?? null);
      }
    } catch {
      /* 取消选择或命令失败时静默 */
    }
  }

  return { logPath, isRecording, logTooltip, toggleLogging };
}
