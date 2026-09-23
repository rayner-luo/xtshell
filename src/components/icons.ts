import { h, type VNode } from "vue";

export const CopyIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("g", { fill: "none", stroke: "currentColor", "stroke-linecap": "round", "stroke-linejoin": "round", "stroke-width": "1.5" }, [
      h("path", { d: "M7.5 14.5c0-3.3 0-4.95 1.025-5.975S11.2 7.5 14.5 7.5s4.95 0 5.975 1.025S21.5 11.2 21.5 14.5s0 4.95-1.025 5.975S17.8 21.5 14.5 21.5s-4.95 0-5.975-1.025S7.5 17.8 7.5 14.5" }),
      h("path", { d: "M7.5 16.5c-1.396 0-2.095 0-2.656-.196a3.5 3.5 0 0 1-2.148-2.148C2.5 13.595 2.5 12.896 2.5 11.5v-2c0-3.3 0-4.95 1.025-5.975S6.2 2.5 9.5 2.5h2c1.396 0 2.095 0 2.656.196a3.5 3.5 0 0 1 2.148 2.148c.196.561.196 1.26.196 2.656" }),
    ]),
  ]);

export const PasteIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("g", { fill: "none", stroke: "currentColor", "stroke-linecap": "round", "stroke-linejoin": "round", "stroke-width": "1.5" }, [
      h("path", { d: "M19.502 13h-9m7-3s3 2.21 3 3c0 .791-3 3-3 3M13.998 2h-5a1.5 1.5 0 1 0 0 3h5a1.5 1.5 0 1 0 0-3" }),
      h("path", { d: "M15.498 3.5c1.554.047 2.48.22 3.121.862c.572.572.772 1.37.842 2.638M7.498 3.5c-1.553.047-2.48.22-3.121.862c-.879.879-.879 2.293-.879 5.121V16c0 2.828 0 4.243.879 5.121C5.255 22 6.67 22 9.498 22h4c2.829 0 4.243 0 5.121-.879c.49-.49.707-1.146.803-2.12" }),
    ]),
  ]);

export const McpIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("g", { fill: "none", stroke: "currentColor", "stroke-linecap": "round", "stroke-linejoin": "round", "stroke-width": "1.5" }, [
      h("path", { d: "m3.5 11.75l8.172-8.171a2.828 2.828 0 1 1 4 4m0 0L9.5 13.75m6.172-6.171a2.828 2.828 0 0 1 4 4l-6.965 6.964a1 1 0 0 0 0 1.414L14 21.25" }),
      h("path", { d: "m17.5 9.75l-6.172 6.171a2.829 2.829 0 0 1-4-4L13.5 5.749" }),
    ]),
  ]);

export const FontIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("path", {
      fill: "none",
      stroke: "currentColor",
      "stroke-linecap": "round",
      "stroke-linejoin": "round",
      "stroke-width": "1.5",
      d: "M3 5.5V6a1.5 1.5 0 0 0 1.5 1.5h.167C5.403 7.5 6 6.903 6 6.167c0-.369.298-.667.667-.667H10.5v13h-1a1.5 1.5 0 0 0 0 3h5a1.5 1.5 0 0 0 0-3h-1v-13h3.833c.369 0 .667.298.667.667c0 .736.597 1.333 1.333 1.333h.167A1.5 1.5 0 0 0 21 6v-.5a3 3 0 0 0-3-3H6a3 3 0 0 0-3 3",
    }),
  ]);

export const EncodingIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("g", { fill: "none", stroke: "currentColor", "stroke-width": "1.5" }, [
      h("circle", { cx: "12", cy: "12", r: "10" }),
      h("path", { "stroke-linejoin": "round", d: "M8 12c0 6 4 10 4 10s4-4 4-10s-4-10-4-10s-4 4-4 10Z" }),
      h("path", { "stroke-linecap": "round", "stroke-linejoin": "round", d: "M21 15H3m18-6H3" }),
    ]),
  ]);

export const FolderIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1.25em",
    height: "1.25em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("path", {
      fill: "none",
      stroke: "currentColor",
      "stroke-linecap": "round",
      "stroke-width": "1.5",
      d: "M8 7h8.75c2.107 0 3.16 0 3.917.506a3 3 0 0 1 .827.827C22 9.09 22 10.143 22 12.25c0 3.511 0 5.267-.843 6.528a5 5 0 0 1-1.38 1.38C18.518 21 16.762 21 13.25 21H12c-4.714 0-7.071 0-8.536-1.465C2 18.072 2 15.715 2 11V7.944c0-1.816 0-2.724.38-3.406A3 3 0 0 1 3.538 3.38C4.22 3 5.128 3 6.944 3C8.108 3 8.69 3 9.2 3.191c1.163.436 1.643 1.493 2.168 2.542L12 7",
    }),
  ]);

export const AboutIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("g", { fill: "none", stroke: "currentColor", "stroke-linecap": "round", "stroke-linejoin": "round", "stroke-width": "1.5" }, [
      h("circle", { cx: "12", cy: "12", r: "10" }),
      h("path", { d: "M12 16v-4m-.125-3.75H12m-.25 0a.25.25 0 1 1 .5 0a.25.25 0 0 1-.5 0" }),
    ]),
  ]);

export const ExitIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 1024 1024",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h1024v1024H0z", fill: "none" }),
    h("path", {
      fill: "currentColor",
      d: "M195.2 195.2a64 64 0 0 1 90.5 0L512 421.5l226.3-226.3a64 64 0 0 1 90.5 90.5L602.5 512l226.3 226.3a64 64 0 0 1-90.5 90.5L512 602.5L285.7 828.8a64 64 0 0 1-90.5-90.5L421.5 512L195.2 285.7a64 64 0 0 1 0-90.5",
    }),
  ]);

export const SftpIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("path", {
      fill: "none",
      stroke: "currentColor",
      "stroke-linecap": "round",
      "stroke-linejoin": "round",
      "stroke-width": "1.5",
      d: "M15.5 3.5c4.2 0 6 2.4 6 6l-2-1m-11 12c-4.2 0-6-2.4-6-6l2 1m3-10.988l-.32-.642c-.259-.522-.388-.783-.587-.969a1.5 1.5 0 0 0-.498-.309c-.255-.095-.547-.095-1.13-.094c-.904 0-1.356.001-1.696.19a1.5 1.5 0 0 0-.581.582c-.189.34-.189.792-.189 1.697V7.5c0 1.886 0 2.829.586 3.415s1.528.585 3.414.585h2.504c1.4 0 2.1 0 2.604-.335a2 2 0 0 0 .556-.556c.336-.504.336-1.204.336-2.605c0-1.403 0-2.105-.337-2.609a2 2 0 0 0-.557-.556c-.505-.335-1.207-.334-2.61-.33zm9 10.002l.318-.642c.26-.522.389-.783.588-.969a1.5 1.5 0 0 1 .498-.309c.255-.095.546-.095 1.129-.094c.904 0 1.357.001 1.696.19a1.5 1.5 0 0 1 .582.582c.189.34.189.792.189 1.697v2.533c0 1.886 0 2.829-.586 3.414c-.586.586-1.529.586-3.414.586h-2.504c-1.4 0-2.1 0-2.604-.335a2 2 0 0 1-.557-.556c-.335-.504-.335-1.204-.335-2.605c0-1.403 0-2.105.336-2.61a2 2 0 0 1 .558-.556c.505-.335 1.207-.333 2.61-.33z",
    }),
  ]);

export const AddIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("path", {
      fill: "none",
      stroke: "currentColor",
      "stroke-linecap": "round",
      "stroke-linejoin": "round",
      "stroke-width": "1.5",
      d: "M12.001 20.009a1.5 1.5 0 0 0 1.499-1.5v-5.002h5.002a1.499 1.499 0 0 0 0-2.998H13.5V5.507a1.499 1.499 0 1 0-2.998 0v5.002H5.499a1.499 1.499 0 1 0 0 2.998h5.003v5.003c0 .828.671 1.499 1.499 1.499",
    }),
  ]);

export const DisconnectIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("g", { fill: "none", stroke: "currentColor", "stroke-linecap": "round", "stroke-width": "1.5" }, [
      h("path", { d: "m9.524 14.437l4.913-4.914" }),
      h("path", {
        "stroke-linejoin": "round",
        d: "M21.016 16.009h-2.004m-3.005 5.01v-2.004M2.983 7.992h2.004m3.005-5.01v2.004m7.116 7.583c1.14.74 2.308.542 3.163-.312l2.994-2.995a2.505 2.505 0 0 0 0-3.543L18.28 2.733a2.504 2.504 0 0 0-3.543 0l-2.994 2.995c-.702.702-1.107 2.036-.277 3.182m1.103 6.199c.74 1.14.542 2.309-.312 3.163l-2.995 2.995a2.504 2.504 0 0 1-3.542 0l-2.985-2.986a2.505 2.505 0 0 1 0-3.542l2.994-2.996c.702-.701 2.036-1.107 3.182-.277",
      }),
    ]),
  ]);

export const ConnectIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("path", {
      fill: "none",
      stroke: "currentColor",
      "stroke-linecap": "round",
      "stroke-width": "1.5",
      d: "M4.513 19.487c2.512 2.392 5.503 1.435 6.7.466c.618-.501.897-.825 1.136-1.065c.837-.777.784-1.555.24-2.177c-.219-.249-1.616-1.591-2.956-2.967c-.694-.694-1.172-1.184-1.582-1.58c-.547-.546-1.026-1.172-1.744-1.154c-.658 0-1.136.58-1.735 1.179c-.688.688-1.196 1.555-1.375 2.333c-.539 2.273.299 3.888 1.316 4.965Zm0 0L2 21.999M19.487 4.515c-2.513-2.394-5.494-1.42-6.69-.45c-.62.502-.898.826-1.138 1.066c-.837.778-.784 1.556-.239 2.178c.078.09.31.32.635.644m7.432-3.438c1.017 1.077 1.866 2.71 1.327 4.985c-.18.778-.688 1.645-1.376 2.334c-.598.598-1.077 1.179-1.735 1.179c-.718.018-1.09-.502-1.639-1.048m3.423-7.45L22 2m-5.936 9.964c-.41-.395-.994-.993-1.688-1.687c-.858-.882-1.74-1.75-2.321-2.325m4.009 4.012l-1.562 1.525m-3.99-3.984l1.543-1.553",
    }),
  ]);

export const LogIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("circle", {
      cx: "12",
      cy: "12",
      r: "8",
      class: "log-target",
      fill: "none",
      stroke: "currentColor",
      "stroke-linejoin": "round",
      "stroke-width": "1.5",
    }),
  ]);

export const ThemeIcon = (): VNode =>
  h("svg", {
    xmlns: "http://www.w3.org/2000/svg",
    width: "1em",
    height: "1em",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  }, [
    h("path", { d: "M0 0h24v24H0z", fill: "none" }),
    h("circle", {
      cx: "12",
      cy: "12",
      r: "9",
      fill: "none",
      stroke: "currentColor",
      "stroke-width": "1.5",
    }),
    // 右半填充：明暗对比的主题图标
    h("path", { d: "M12 3a9 9 0 0 1 0 18Z", fill: "currentColor" }),
  ]);
