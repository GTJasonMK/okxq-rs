# okxQ-rs

OKX quantitative trading desktop app built with Tauri 2, Vue 3, TypeScript, and Rust.

## Stack

- Frontend: Vue 3, TypeScript, Vite, Pinia, Vitest.
- Desktop shell: Tauri 2.
- Backend: Rust, SQLite via SQLx, OKX REST/WebSocket integrations.

## Repository Layout

- `src/`: Vue frontend, composables, API clients, views, and frontend tests.
- `src-tauri/`: Rust backend, Tauri commands, storage, OKX clients, realtime data, and Rust tests.
- `strategies/`: runtime strategy files loaded by the app.
- `scripts/`: development and smoke test helpers.
- `config/`, `data/`, `logs/`: local runtime areas. Do not commit secrets, preferences, databases, or logs.

Local research and generated model artifacts are intentionally ignored:

- `scripts/research/`
- `strategy_records/`
- `strategies/artifacts/`

## Local Setup

Install Node.js 22+, Rust stable, and platform dependencies required by Tauri.

```bash
npm install
cp config/.env.example config/.env
```

Fill `config/.env` with local OKX credentials as needed. Keep this file private.

## Development

```bash
npm run dev
```

Useful narrow commands:

```bash
npm run dev:frontend
npm run type-check
npm run test:frontend
npm run build:frontend
npm run check:rust
npm run test:rust
```

## GitHub Upload Notes

Before pushing a fresh repository, confirm that no local secrets or generated state are tracked:

```bash
git status --short
git ls-files config/.env config/user_preferences.json src-tauri/python/__pycache__ scripts/research strategy_records strategies/artifacts
```

`config/.env`, `config/user_preferences.json`, local research records, model artifacts, databases, logs, build output, caches, and Python bytecode should stay out of Git. Use `config/.env.example` as the committed template.

## CI

GitHub Actions are configured in `.github/workflows/ci.yml` for frontend type checks/tests/builds and Rust check/tests.

## License

No open-source license has been declared yet.
