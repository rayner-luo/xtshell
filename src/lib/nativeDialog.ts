import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * 弹出系统选择框前，先把主窗口提到前台。
 *
 * ZMODEM 的选择框是由后端事件触发的（没有用户点击手势）：Windows 的前台
 * 激活规则此时可能不让新弹出的模态对话框成为活动窗口，表现为「框弹出来了，
 * 但鼠标点不上去 / 像被主窗口挡住」。先聚焦主窗口，随后的模态框才能正常
 * 取得激活；GTK 下也需先抬起父窗口，否则 transient 对话框可能被压在下面。
 */
async function activateMainWindow() {
  try {
    const win = getCurrentWindow();
    await win.unminimize();
    await win.setFocus();
  } catch {
    /* 激活失败不应阻断选文件 */
  }
  // 给窗口管理器/前台激活留出时间，再弹模态框
  await new Promise((r) => setTimeout(r, 120));
}

/** 选目录（ZMODEM 接收等） */
export async function pickDirectory(): Promise<string | null> {
  await activateMainWindow();
  const dir = await open({ directory: true });
  return typeof dir === "string" ? dir : null;
}

/** 选一个或多个文件（ZMODEM 发送等） */
export async function pickFiles(): Promise<string[] | null> {
  await activateMainWindow();
  const paths = await open({ multiple: true });
  if (!paths) return null;
  return Array.isArray(paths) ? paths : [paths];
}