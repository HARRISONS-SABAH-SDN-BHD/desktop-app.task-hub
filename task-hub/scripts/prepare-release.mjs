import {
  copyFileSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";

export function releaseManifest({
  version,
  signature,
  repository,
  assetName,
  notes,
  date,
}) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version))
    throw new Error("Use a valid release version");
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository))
    throw new Error("Use owner/repository for GitHub");
  if (!signature || !/^[A-Za-z0-9+/=\r\n]+$/.test(signature))
    throw new Error("A Tauri signature is required");
  if (!assetName.endsWith(".exe") || /[\\/]/.test(assetName))
    throw new Error("Use an installer filename");
  const decoded = Buffer.from(signature, "base64").toString("utf8");
  if (
    !decoded.includes(`version:${version}`) &&
    !decoded.includes(`version=${version}`)
  ) {
    // Current Tauri signatures bind the app version in a signed trusted comment.
    if (!decoded.includes(`version ${version}`))
      throw new Error(
        "Signature must be bound to this app version. Use a current Tauri CLI to build the release.",
      );
  }
  return {
    version,
    notes,
    pub_date: date,
    platforms: {
      "windows-x86_64": {
        signature: signature.trim(),
        url: `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(assetName)}`,
      },
    },
  };
}
export function prepareRelease({
  input,
  output,
  repository = "devDaniel0330/task-hub",
}) {
  const pkg = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url)),
  );
  const config = JSON.parse(
    readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url)),
  );
  if (
    pkg.version !== config.version ||
    config.identifier !== "com.taskhub.desktop"
  )
    throw new Error("Release versions/identifier do not match");
  const files = readdirSync(input).filter(
    (name) => name.endsWith(".exe") && name.includes(pkg.version),
  );
  if (files.length !== 1)
    throw new Error("Expected exactly one version-matched NSIS installer");
  const source = join(input, files[0]);
  const signature = readFileSync(`${source}.sig`, "utf8").trim();
  const assetName = `TaskHub-${pkg.version}-windows-x64-setup.exe`;
  const notes = `Task Hub ${pkg.version}\nSigned automatic updates. Existing routines, email delivery, attachments and Google Calendar sync are preserved. Save your work before installing an update.`;
  const manifest = releaseManifest({
    version: pkg.version,
    signature,
    repository,
    assetName,
    notes,
    date: new Date().toISOString(),
  });
  mkdirSync(output, { recursive: true });
  copyFileSync(source, join(output, assetName));
  copyFileSync(`${source}.sig`, join(output, `${assetName}.sig`));
  writeFileSync(
    join(output, "latest.json"),
    JSON.stringify(manifest, null, 2) + "\n",
  );
  writeFileSync(join(output, "release-notes.txt"), notes + "\n");
  writeFileSync(
    join(output, "SHA256SUMS.txt"),
    `${createHash("sha256").update(readFileSync(source)).digest("hex")}  ${assetName}\n`,
  );
  writeFileSync(
    join(output, "Publish-updates.txt"),
    `Task Hub release ${pkg.version}\n\nFirst installation: share ${assetName} with testers. Each tester connects their own Google account.\n\nPublish all five release assets (${assetName}, its .sig, latest.json, SHA256SUMS.txt, release-notes.txt) in the public GitHub release v${pkg.version} at ${repository}. Mark it as the latest release. The installed app reads /releases/latest/download/latest.json. Draft/private releases are not available through that public feed.\n\nFuture updates: bump package.json, package-lock.json, Cargo.toml/Cargo.lock and tauri.conf.json to the same newer version; run tests; build with the SAME private signing key; publish the new version-tagged release. Never overwrite a published version or commit/upload the private signing key.\n\nGitHub Actions: add repository secrets TAURI_UPDATER_SIGNING_KEY (the PRIVATE key file contents) and optionally TAURI_UPDATER_SIGNING_PASSWORD. Use the Release installer workflow or push a matching vVERSION tag. Only the public key is included in the app. Keep a secure backup of the private key.\n\nUpdater signatures authenticate updates; they are separate from a Windows Authenticode publisher certificate. This test installer is not Authenticode-signed.\n`,
  );
  return {
    version: pkg.version,
    installer: join(output, assetName),
    manifest: join(output, "latest.json"),
  };
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const input =
    process.argv[2] ??
    resolve(
      process.env.CARGO_TARGET_DIR ?? "src-tauri/target",
      "release/bundle/nsis",
    );
  const output = resolve(process.argv[3] ?? "release");
  try {
    console.log(JSON.stringify(prepareRelease({ input, output })));
  } catch (e) {
    console.error(e.message);
    process.exitCode = 1;
  }
}
