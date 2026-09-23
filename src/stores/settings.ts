import { defineStore } from "pinia";
import { ref, watch } from "vue";
import { setNativeTheme } from "../lib/api";

const STORAGE_KEY = "xtshell.settings";

export type ThemeName = "dark" | "light";

export const fontOptions = [
  { label: "Cascadia Mono", value: '"Cascadia Mono", Consolas, monospace' },
  { label: "Consolas", value: 'Consolas, monospace' },
  { label: "Courier New", value: '"Courier New", monospace' },
  { label: "JetBrains Mono", value: '"JetBrains Mono", Consolas, monospace' },
  { label: "Fira Code", value: '"Fira Code", Consolas, monospace' },
  { label: "DejaVu Sans Mono", value: '"DejaVu Sans Mono", Consolas, monospace' },
];

interface SavedSettings {
  fontFamily: string;
  fontSize: number;
  theme: ThemeName;
}

function load(): SavedSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        fontFamily: typeof parsed.fontFamily === "string" ? parsed.fontFamily : fontOptions[0].value,
        fontSize:
          typeof parsed.fontSize === "number" && parsed.fontSize >= 8 && parsed.fontSize <= 32
            ? parsed.fontSize
            : 13,
        theme: parsed.theme === "dark" ? "dark" : "light",
      };
    }
  } catch {
    /* ignore */
  }
  return { fontFamily: fontOptions[0].value, fontSize: 13, theme: "light" };
}

/** 把主题同步到 DOM（CSS 变量切换）与原生层（Linux 弹出菜单 GTK 主题） */
function applyTheme(theme: ThemeName) {
  document.documentElement.dataset.theme = theme;
  setNativeTheme(theme === "dark").catch(() => {});
}

export const useSettingsStore = defineStore("settings", () => {
  const saved = load();
  const fontFamily = ref<string>(saved.fontFamily);
  const fontSize = ref<number>(saved.fontSize);
  const theme = ref<ThemeName>(saved.theme);

  watch(
    [fontFamily, fontSize, theme],
    () => {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ fontFamily: fontFamily.value, fontSize: fontSize.value, theme: theme.value }),
      );
    },
    { deep: true },
  );

  // 初始也要同步原生层：持久化为深色的用户启动时 GTK 主题需立即跟随
  // （gtk-theme-name 运行时写入已实测不影响下拉弹出的创建）。
  watch(theme, (t) => applyTheme(t));
  applyTheme(theme.value);

  function setTheme(t: ThemeName) {
    theme.value = t;
  }

  return { fontFamily, fontSize, theme, setTheme };
});
