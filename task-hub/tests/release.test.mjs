import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { releaseManifest } from "../scripts/prepare-release.mjs";
const signature = Buffer.from(
  "untrusted comment: fixture\nfixture\ntrusted comment: timestamp:1\tversion:0.3.0\nfixture",
).toString("base64");
const args = {
  version: "0.3.0",
  signature,
  repository: "devDaniel0330/task-hub",
  assetName: "TaskHub-0.3.0-windows-x64-setup.exe",
  notes: "Test release",
  date: "2026-11-05T00:00:00Z",
};
test("release feed selects the Windows x64 signed installer and an immutable version URL", () => {
  const result = releaseManifest(args);
  assert.equal(result.version, "0.3.0");
  assert.equal(result.platforms["windows-x86_64"].signature, signature);
  assert.equal(
    result.platforms["windows-x86_64"].url,
    "https://github.com/devDaniel0330/task-hub/releases/download/v0.3.0/TaskHub-0.3.0-windows-x64-setup.exe",
  );
});
test("missing signatures, version mismatches and unsafe filenames cannot be published", () => {
  for (const change of [
    { signature: "" },
    { version: "0.4.0" },
    { assetName: "../private.key.exe" },
    { repository: "https://example.com" },
  ])
    assert.throws(() => releaseManifest({ ...args, ...change }));
});
test("application identity and signed update policy stay stable across packaging", () => {
  const config = JSON.parse(
    readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url)),
  );
  assert.equal(config.identifier, "com.taskhub.desktop");
  assert.equal(
    config.version,
    JSON.parse(readFileSync(new URL("../package.json", import.meta.url)))
      .version,
  );
  assert.equal(config.plugins.updater.requireSignedVersion, true);
  assert.equal(config.plugins.updater.allowDowngrades, false);
  assert.equal(config.bundle.windows.nsis.installMode, "currentUser");
  assert.equal(
    config.plugins.updater.endpoints[0],
    "https://github.com/devDaniel0330/task-hub/releases/latest/download/latest.json",
  );
  const publicKey = Buffer.from(
    config.plugins.updater.pubkey,
    "base64",
  ).toString("utf8");
  assert.ok(publicKey.includes("public key"));
  assert.ok(!publicKey.includes("secret key"));
  assert.deepEqual(config.bundle.resources, {
    "resources/runtime/taskhub-runtime.exe": "runtime/taskhub-runtime.exe",
    "resources/runtime/settings.json": "runtime/settings.json",
    "resources/runtime/report_styles.json": "runtime/report_styles.json",
  });
});
