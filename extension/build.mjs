// Builds dist/chromium and dist/firefox, each loadable unpacked.
//
// The manifests in manifests/ may have whole-line // comments, which no
// browser takes: they are taken out here. Each gets the version from
// package.json. With --store, the Chromium manifest leaves out "key", which
// the stores do not take.

import { build } from "esbuild";
import { copyFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";

const store = process.argv.includes("--store");
const pkg = JSON.parse(await readFile("package.json", "utf8"));

const targets = {
  chromium: "chrome120",
  firefox: "firefox140",
};

for (const [name, target] of Object.entries(targets)) {
  const out = `dist/${name}`;
  await rm(out, { recursive: true, force: true });
  await mkdir(out, { recursive: true });
  await build({
    entryPoints: ["src/background.ts", "src/content.ts", "src/popup.ts"],
    outdir: out,
    bundle: true,
    // Content scripts, Firefox's background scripts and a classic service
    // worker all take plain scripts, not modules.
    format: "iife",
    target,
    define: { __BROWSER__: JSON.stringify(name) },
    logLevel: "warning",
  });
  await copyFile("src/popup.html", `${out}/popup.html`);
  const source = await readFile(`manifests/${name}.json`, "utf8");
  const manifest = JSON.parse(
    source
      .split("\n")
      .filter((line) => !line.trimStart().startsWith("//"))
      .join("\n"),
  );
  manifest.version = pkg.version;
  if (store) delete manifest.key;
  await writeFile(`${out}/manifest.json`, JSON.stringify(manifest, null, 2) + "\n");
  console.log(`${out} (${manifest.version}${store ? ", for the store" : ""})`);
}
