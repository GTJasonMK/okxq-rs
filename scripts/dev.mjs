#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const runtimeDir = path.join(repoRoot, '.dev-runtime');
const pidFile = path.join(runtimeDir, 'dev.pid');
const processFile = path.join(runtimeDir, 'processes.json');
const tauriConfigFile = path.join(runtimeDir, 'tauri.dev.conf.json');
const logFile = path.join(runtimeDir, 'dev.log');

const host = process.env.OKXQ_DEV_HOST || process.env.TAURI_DEV_HOST || '127.0.0.1';
const preferredPort = Number.parseInt(process.env.OKXQ_DEV_PORT || process.env.PORT || '5173', 10);
const maxPort = Number.parseInt(process.env.OKXQ_DEV_PORT_MAX || '5299', 10);
const rustLog = process.env.RUST_LOG || 'okxq_rs=debug,info';
const frontendDebug = process.env.VITE_OKXQ_DEBUG || 'true';
const devArgs = process.argv.slice(2);
const noHotReloadFlags = new Set(['--no-hot-reload', '--no-hmr']);
const noHotReload = devArgs.some((arg) => noHotReloadFlags.has(arg))
  || ['1', 'true', 'yes', 'on'].includes(String(process.env.OKXQ_DEV_NO_HOT_RELOAD || '').toLowerCase());
const extraArgs = devArgs.filter((arg) => !noHotReloadFlags.has(arg));
const viteBin = process.platform === 'win32'
  ? path.join(repoRoot, 'node_modules', '.bin', 'vite.cmd')
  : path.join(repoRoot, 'node_modules', '.bin', 'vite');
const tauriBin = process.platform === 'win32'
  ? path.join(repoRoot, 'node_modules', '.bin', 'tauri.cmd')
  : path.join(repoRoot, 'node_modules', '.bin', 'tauri');

const children = new Map();
let shuttingDown = false;

const log = (message) => {
  const line = `[dev] ${message}`;
  console.log(line);
};

const isPortFree = (port) => new Promise((resolve, reject) => {
  const server = net.createServer();
  server.once('error', (error) => {
    if (error?.code === 'EADDRINUSE') {
      resolve(false);
      return;
    }
    reject(error);
  });
  server.once('listening', () => {
    server.close(() => resolve(true));
  });
  server.listen({ host, port, exclusive: true });
});

const findFreePort = async (startPort) => {
  for (let port = startPort; port <= maxPort; port += 1) {
    if (await isPortFree(port)) {
      return port;
    }
  }
  throw new Error(`No free port found on ${host}:${startPort}-${maxPort}`);
};

const waitForHttp = async (url, timeoutMs = 45_000) => {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, { cache: 'no-store' });
      if (response.ok || response.status < 500) {
        return;
      }
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => {
      setTimeout(resolve, 250);
    });
  }
  throw new Error(`Timed out waiting for ${url}${lastError ? `: ${lastError.message}` : ''}`);
};

const spawnChild = (name, command, args, env = {}) => {
  const child = spawn(command, args, {
    cwd: repoRoot,
    env: { ...process.env, ...env },
    stdio: 'inherit',
    detached: process.platform !== 'win32',
  });

  children.set(name, child);
  void writeProcessFile();
  child.once('exit', (code, signal) => {
    children.delete(name);
    void writeProcessFile();
    if (!shuttingDown) {
      const reason = signal ? `signal ${signal}` : `code ${code}`;
      log(`${name} exited unexpectedly with ${reason}`);
      void shutdown(code || 1);
    }
  });
  child.once('error', (error) => {
    children.delete(name);
    void writeProcessFile();
    if (!shuttingDown) {
      log(`${name} failed to start: ${error.message}`);
      void shutdown(1);
    }
  });
  return child;
};

const processExists = (pid) => {
  if (!Number.isFinite(pid) || pid <= 0 || pid === process.pid) {
    return false;
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === 'EPERM';
  }
};

const waitForExit = async (pid, timeoutMs = 1_500) => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!processExists(pid)) {
      return;
    }
    await new Promise((resolve) => {
      setTimeout(resolve, 100);
    });
  }
};

const terminatePid = async (pid, name = 'process') => {
  if (!processExists(pid)) {
    return;
  }
  log(`stopping stale ${name} pid ${pid}...`);
  try {
    process.kill(pid, 'SIGTERM');
  } catch {
    return;
  }
  await waitForExit(pid);
  if (!processExists(pid)) {
    return;
  }
  try {
    process.kill(pid, 'SIGKILL');
  } catch {
    // Already exited.
  }
};

const readPidFile = async () => {
  try {
    const raw = await readFile(pidFile, 'utf8');
    const pid = Number.parseInt(raw.trim(), 10);
    return Number.isFinite(pid) && pid > 0 ? pid : null;
  } catch {
    return null;
  }
};

const readProcessFile = async () => {
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

const writeProcessFile = async () => {
  const payload = {
    parent: process.pid,
    children: [...children.entries()].map(([name, child]) => ({
      name,
      pid: child.pid ?? null,
    })),
    updatedAt: new Date().toISOString(),
  };
  try {
    await writeFile(processFile, `${JSON.stringify(payload, null, 2)}\n`);
  } catch {
    // Best-effort diagnostic file only.
  }
};

const stopChild = async (name, child) => {
  if (!child || child.killed) {
    return;
  }
  log(`stopping ${name}...`);
  const targetPid = process.platform === 'win32' ? child.pid : -child.pid;
  try {
    process.kill(targetPid, 'SIGTERM');
  } catch {
    return;
  }
  await new Promise((resolve) => {
    const timer = setTimeout(resolve, 4_000);
    child.once('exit', () => {
      clearTimeout(timer);
      resolve();
    });
  });
  if (!child.killed && child.exitCode == null) {
    try {
      process.kill(targetPid, 'SIGKILL');
    } catch {
      // Already exited.
    }
  }
};

const cleanupRuntimeFiles = async () => {
  await rm(pidFile, { force: true });
  await rm(processFile, { force: true });
  await rm(tauriConfigFile, { force: true });
};

const cleanupStaleRuntimeFiles = async () => {
  const pid = await readPidFile();
  const processes = await readProcessFile();
  const parent = processes.parent || pid;
  if (parent && parent !== process.pid) {
    await terminatePid(parent, 'dev supervisor');
  }
  for (const child of processes.children) {
    await terminatePid(child.pid, child.name);
  }
  await cleanupRuntimeFiles();
};

const shutdown = async (exitCode = 0) => {
  if (shuttingDown) {
    return;
  }
  shuttingDown = true;
  const entries = [...children.entries()].reverse();
  await Promise.all(entries.map(([name, child]) => stopChild(name, child)));
  await cleanupRuntimeFiles();
  process.exit(exitCode);
};

const main = async () => {
  await mkdir(runtimeDir, { recursive: true });
  await cleanupStaleRuntimeFiles();
  const port = await findFreePort(Number.isFinite(preferredPort) ? preferredPort : 5173);
  const devUrl = `http://${host}:${port}`;
  const tauriConfig = {
    build: {
      beforeDevCommand: '',
      devUrl,
    },
  };

  await writeFile(tauriConfigFile, `${JSON.stringify(tauriConfig, null, 2)}\n`);
  await writeFile(pidFile, `${process.pid}\n`);
  await writeProcessFile();
  await writeFile(logFile, `started=${new Date().toISOString()}\nhost=${host}\nport=${port}\ndevUrl=${devUrl}\n`);

  log(`frontend dev server: ${devUrl}`);
  log(`runtime files: ${path.relative(repoRoot, runtimeDir)}`);
  if (noHotReload) {
    log('hot reload disabled: Vite HMR off, Tauri watcher off');
  }

  const viteArgs = [
    '--host',
    host,
    '--port',
    String(port),
    '--strictPort',
    '--clearScreen',
    'false',
  ];
  spawnChild('vite', viteBin, viteArgs, {
    TAURI_DEV_HOST: host,
    VITE_DEV_SERVER_URL: devUrl,
    PORT: String(port),
    VITE_OKXQ_DEBUG: frontendDebug,
    VITE_OKXQ_DISABLE_HMR: noHotReload ? 'true' : '',
  });

  await waitForHttp(devUrl);

  const configArg = path.relative(repoRoot, tauriConfigFile);
  const tauriArgs = ['dev', '--config', configArg, '--no-dev-server-wait'];
  if (noHotReload && !extraArgs.includes('--no-watch')) {
    tauriArgs.push('--no-watch');
  }
  if (extraArgs.length > 0) {
    tauriArgs.push(...extraArgs);
  }

  spawnChild('tauri', tauriBin, tauriArgs, {
    TAURI_DEV_HOST: host,
    VITE_DEV_SERVER_URL: devUrl,
    PORT: String(port),
    RUST_LOG: rustLog,
    VITE_OKXQ_DEBUG: frontendDebug,
    VITE_OKXQ_DISABLE_HMR: noHotReload ? 'true' : '',
  });
};

process.once('SIGINT', () => {
  void shutdown(130);
});
process.once('SIGTERM', () => {
  void shutdown(143);
});
process.once('SIGHUP', () => {
  void shutdown(129);
});
process.once('uncaughtException', (error) => {
  console.error(error);
  void shutdown(1);
});
process.once('unhandledRejection', (error) => {
  console.error(error);
  void shutdown(1);
});

main().catch((error) => {
  console.error(error);
  void shutdown(1);
});
