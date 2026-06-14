#!/usr/bin/env node

import { readFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const runtimeDir = path.join(repoRoot, '.dev-runtime');
const pidFile = path.join(runtimeDir, 'dev.pid');
const processFile = path.join(runtimeDir, 'processes.json');
const tauriConfigFile = path.join(runtimeDir, 'tauri.dev.conf.json');

const readPid = async () => {
  try {
    const raw = await readFile(pidFile, 'utf8');
    const pid = Number.parseInt(raw.trim(), 10);
    return Number.isFinite(pid) && pid > 0 ? pid : null;
  } catch {
    return null;
  }
};

const readProcesses = async () => {
  try {
    const raw = await readFile(processFile, 'utf8');
    const parsed = JSON.parse(raw);
    const parent = Number.parseInt(String(parsed?.parent ?? ''), 10);
    const children = Array.isArray(parsed?.children)
      ? parsed.children
        .map((item) => ({
          name: String(item?.name || 'child'),
          pid: Number.parseInt(String(item?.pid ?? ''), 10),
        }))
        .filter((item) => Number.isFinite(item.pid) && item.pid > 0)
      : [];
    return {
      parent: Number.isFinite(parent) && parent > 0 ? parent : null,
      children,
    };
  } catch {
    return { parent: null, children: [] };
  }
};

const killProcessGroup = async (pid) => {
  if (!pid) {
    return;
  }
  const targetPid = process.platform === 'win32' ? pid : -pid;
  try {
    process.kill(targetPid, 'SIGTERM');
  } catch {
    return;
  }
  await new Promise((resolve) => {
    setTimeout(resolve, 1500);
  });
  try {
    process.kill(targetPid, 'SIGKILL');
  } catch {
    // Already exited.
  }
};

const main = async () => {
  const pid = await readPid();
  const processes = await readProcesses();
  if (pid) {
    console.log(`[dev:cleanup] stopping process group for pid ${pid}`);
    await killProcessGroup(pid);
  }
  for (const child of processes.children) {
    console.log(`[dev:cleanup] stopping stale ${child.name} pid ${child.pid}`);
    await killProcessGroup(child.pid);
  }
  if (processes.parent && processes.parent !== pid) {
    console.log(`[dev:cleanup] stopping stale dev supervisor pid ${processes.parent}`);
    await killProcessGroup(processes.parent);
  }
  await rm(pidFile, { force: true });
  await rm(processFile, { force: true });
  await rm(tauriConfigFile, { force: true });
  console.log('[dev:cleanup] runtime files cleaned');
};

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
