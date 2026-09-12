import { defineStore } from "pinia";
import { ref, watch } from "vue";

const STORAGE_KEY = "xtshell.settings";

export const fontOptions = [
  { label: "Cascadia Mono", value: '"Cascadia Mono", Consolas, monospace' },
  { label: "Consolas", value: 'Consolas, monospace' },
  { label: "Courier New", value: '"Courier New", monospace' },
  { label: "JetBrains Mono", value: '"JetBrains Mono", Consolas, monospace' },
  { label: "Fira Code", value: '"Fira Code", Consolas, monospace' },
  { label: "DejaVu Sans Mono", value: '"DejaVu Sans Mono", Consolas, monospace' },
];

function load(): { fontFamily: string; fontSize: number } {
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
      };
    }
  } catch {
    /* ignore */
  }
  return { fontFamily: fontOptions[0].value, fontSize: 13 };
}

export const useSettingsStore = defineStore("settings", () => {
  const saved = load();
  const fontFamily = ref<string>(saved.fontFamily);
  const fontSize = ref<number>(saved.fontSize);

  watch(
    [fontFamily, fontSize],
    () => {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ fontFamily: fontFamily.value, fontSize: fontSize.value }),
      );
    },
    { deep: true },
  );

  return { fontFamily, fontSize };
});
