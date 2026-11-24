import { pickFile } from "./nativeDialog";

/** 当前 WebView 是否运行在 Windows（.exe 过滤只在 Windows 启用） */
export function isWindowsPlatform(): boolean {
  return /Windows/i.test(navigator.userAgent);
}

/** 程序路径规范：去首尾空白与一对包围引号 */
export function normalizeShellPath(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed.length >= 2) {
    const first = trimmed[0];
    const last = trimmed[trimmed.length - 1];
    if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
      return trimmed.slice(1, -1).trim();
    }
  }
  return trimmed;
}

/** 文件对话框起始目录：当前路径的父目录，否则系统默认目录 */
export function shellStartDir(raw: string, windows = isWindowsPlatform()): string | undefined {
  const path = normalizeShellPath(raw);
  if (path) {
    const sep = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
    if (sep > 0) {
      let dir = path.slice(0, sep);
      // 根目录 "C:" 需要补回分隔符
      if (windows && /^[a-zA-Z]:$/.test(dir)) dir += "\\";
      return dir;
    }
  }
  return windows ? "C:\\Windows\\System32" : "/bin";
}

/** 默认会话名：取路径最后一段（如 powershell.exe），空路径用「本地 Shell」 */
export function shellDisplayName(raw: string): string {
  const path = normalizeShellPath(raw);
  if (!path) return "本地 Shell";
  const idx = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return idx >= 0 ? path.slice(idx + 1) : path;
}

/** 弹系统文件选择框选 Shell 程序；取消返回 null */
export async function pickShellPath(current: string): Promise<string | null> {
  const windows = isWindowsPlatform();
  const defaultPath = shellStartDir(current, windows);
  return pickFile({
    title: "选择 Shell 程序",
    ...(defaultPath ? { defaultPath } : {}),
    ...(windows ? { filters: [{ name: "可执行文件", extensions: ["exe"] }] } : {}),
  });
}
