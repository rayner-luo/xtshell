// 打包前对编译产物做 UPX 压缩。
// Windows / Linux 生效；macOS 跳过（UPX 会破坏代码签名，arm64 上还有兼容性问题）。
// 由 tauri.conf.json 的 build.beforeBundleCommand 调用，因此对安装包内的二进制同样生效。
import { execFileSync } from "node:child_process";
import { readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const targetRoot = join(root, "src-tauri", "target");
const isWin = process.platform === "win32";
// 交叉编译时宿主机平台与产物后缀可能不一致（Linux 上产出 .exe），两种都试
const binaries = isWin ? ["xtshell.exe"] : ["xtshell", "xtshell.exe"];

if (process.platform === "darwin") {
  console.log("[upx] macOS：跳过压缩，保持原始打包产物");
  process.exit(0);
}

// 解析可用的 upx 命令：Windows/choco 提供 upx，Debian/Ubuntu 的包名是 upx-ucl
function resolveUpx() {
  const candidates = [process.env.UPX_BIN, "upx", "upx-ucl"].filter(Boolean);
  for (const candidate of candidates) {
    try {
      execFileSync(candidate, ["--version"], { stdio: "ignore" });
      return candidate;
    } catch {
      /* 试下一个 */
    }
  }
  return null;
}

const upx = resolveUpx();
if (!upx) {
  console.warn("[upx] 未找到 upx/upx-ucl，跳过压缩（安装包仍可正常产出）");
  process.exit(0);
}
console.log(`[upx] 使用命令: ${upx}`);

// 候选目录：target/release 与 target/<triple>/release
const dirs = [join(targetRoot, "release")];
try {
  for (const entry of readdirSync(targetRoot, { withFileTypes: true })) {
    if (entry.isDirectory() && entry.name !== "release") {
      dirs.push(join(targetRoot, entry.name, "release"));
    }
  }
} catch {
  /* target 目录尚未创建 */
}

let packed = 0;
let seen = 0;
for (const dir of dirs) {
  for (const binary of binaries) {
    const file = join(dir, binary);
    let size = 0;
    try {
      const stat = statSync(file);
      if (!stat.isFile()) continue;
      size = stat.size;
    } catch {
      continue;
    }
    seen += 1;
    console.log(`[upx] 压缩 ${file}（${(size / 1048576).toFixed(1)} MB）`);
    try {
      execFileSync(upx, ["--best", "--lzma", file], { stdio: "inherit" });
      const after = statSync(file).size;
      console.log(
        `[upx] 完成 ${(after / 1048576).toFixed(1)} MB（${((after / size) * 100).toFixed(1)}%）`,
      );
      packed += 1;
    } catch (e) {
      // 已压缩过或不支持的格式：只告警，不阻塞打包
      const reason = e instanceof Error ? e.message.split("\n")[0] : String(e);
      console.warn(`[upx] 跳过 ${file}：${reason}`);
    }
  }
}

if (seen === 0) {
  console.warn(`[upx] 未找到可压缩的产物（${binaries.join(" / ")}）`);
} else {
  console.log(`[upx] 处理完成：压缩 ${packed} 个，跳过 ${seen - packed} 个`);
}
