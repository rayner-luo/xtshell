import { computed } from "vue";
import { useSessionStore } from "../stores/session";
import { setSessionLog } from "../lib/api";
import { open as pickDir } from "@tauri-apps/plugin-dialog";

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
        const dir = await pickDir({
          directory: true,
          title: "选择会话日志保存目录",
        });
        if (!dir) return;
        const path = await setSessionLog(id, dir);
        store.setLogging(id, path ?? null);
      }
    } catch {
      /* 选目录取消或命令失败时静默 */
    }
  }

  return { logPath, isRecording, logTooltip, toggleLogging };
}
