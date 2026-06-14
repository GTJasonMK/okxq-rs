# Repository Guidelines

## Project Structure & Module Organization

This is a Tauri 2 desktop app with a Vue 3/TypeScript frontend and a Rust backend.

- `src/`: frontend app: `components/`, `views/`, `composables/`, `router/`, `config/`, `styles/`, and `src/__tests__/`.
- `src-tauri/src/`: backend modules, Tauri commands, storage, OKX integrations, strategy execution, realtime data, and Rust tests.
- `src-tauri/icons/`: app icons.
- `scripts/`: development helpers, smoke tests, and Python research tooling.
- `strategies/`: runtime strategy files loaded by the app.
- `strategy_records/`: archived strategy source and research records used for reproduction.
- `config/`, `data/`, `logs/`: runtime areas; avoid committing generated or sensitive local state.

## Build, Test, and Development Commands

- `npm run dev`: start coordinated local development.
- `npm run dev:frontend`: run Vite only on `127.0.0.1`.
- `npm run dev:tauri`: run Tauri shell.
- `npm run build:frontend`: build frontend assets.
- `npm run build`: create production Tauri build.
- `npm run type-check`: run `vue-tsc --noEmit`.
- `npm run test:frontend`: run Vitest once.
- `npm run check:rust`: run Cargo check for `src-tauri`.
- `npm run test:rust`: run Rust tests.
- `npm run smoke:gui`: run GUI smoke checks.

## Coding Style & Naming Conventions

Use TypeScript, Vue SFCs, and Rust idioms already present. Frontend files use 2-space indentation, single quotes, PascalCase Vue components such as `DataCenterHeader.vue`, and composables named `useXxx.ts`. Keep shared imports on the `@/` alias. Rust follows `rustfmt`: snake_case functions, PascalCase types, and explicit error propagation.

## Testing Guidelines

Frontend tests use Vitest with jsdom in `src/__tests__/**/*.test.ts`. Match the existing `feature-behavior.test.ts` style and focus on visible state, normalization, and runtime behavior. Rust tests are colocated under `src-tauri/src/**/tests.rs` or `tests/` submodules. Python research tests live under `scripts/research/**/tests/`.

Run the narrowest relevant test first, then broaden to `test:frontend`, `type-check`, `check:rust`, or `test:rust`.

## Commit & Pull Request Guidelines

Recent history uses short subjects such as `bug fixed`, `策略研发`, and `init`; no strict convention is enforced. Prefer clearer imperative subjects, for example `fix: normalize live order status` or `research: add gap audit entrypoint`.

Pull requests should include purpose, key files changed, validation commands, linked issues if any, and screenshots or recordings for visible UI changes. Call out config, data, or strategy-behavior changes explicitly.

## Security & Configuration Tips

Do not hard-code OKX keys, tokens, or private configuration. Use the existing config flow or environment variables, keep local secrets out of commits, and avoid generated data, logs, build output, or cache files unless they are intentional fixtures.
