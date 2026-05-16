# Cockpit Tools Codemap

## Maintenance Rule

- This codemap is a present-tense architecture snapshot for future coding tasks.
- Update it when system boundaries, runtime sources of truth, persistence, routes, local services, env contracts, integrations, release assumptions, or gating rules change.
- Keep migration notes, changelog entries, and release diary material out of this file.
- Add `docs/codemap-*.md` only when a subsystem needs detail that would bloat this top-level map.

## Product Boundary

- Product name: Cockpit Tools.
- Primary repo path: `/Users/alen/Shrimpfall-Goose/Code/cockpit-tools`.
- Desktop app identifier: `com.jlcodes.cockpit-tools`.
- App shell: Tauri 2 desktop app with a React 19 + Vite 7 frontend and a Rust backend.
- Code-defined platform IDs: `antigravity`, `codex`, `zed`, `github-copilot`, `windsurf`, `kiro`, `cursor`, `gemini`, `codebuddy`, `codebuddy_cn`, `qoder`, `trae`, `workbuddy`.
- README support boundary: macOS, Windows, and Linux are official support targets.
- Hosted production domain: unknown from repo. Distribution points verified from repo are GitHub Releases and the Homebrew Cask in `Casks/cockpit-tools.rb`.

## What This Project Owns

- The Cockpit Tools desktop UI, window lifecycle, tray/floating-card windows, settings, local logs, backup/import/export, release scripts, and updater integration.
- Local account inventory, tags, notes, quota snapshots, provider account groups, provider layout, and dashboard state for the supported AI IDE/platform accounts.
- Local switching/injection flows that write managed account state into third-party desktop apps and CLI homes.
- Multi-instance profile stores and process launch/stop/open flows for supported platforms.
- Antigravity wakeup, wakeup verification, Codex wakeup, Codex session visibility/thread sync helpers, and Codex local access gateway.
- Local service surfaces used by extensions or local clients: WebSocket, web report, OAuth callback listeners, and Codex OpenAI-compatible access.

## What This Project Does Not Own

- Vendor identity, billing, quota policy, OAuth consent screens, and remote APIs for Antigravity, Codex/OpenAI, GitHub Copilot, Windsurf, Kiro, Cursor, Gemini, CodeBuddy, Qoder, Trae, WorkBuddy, or Zed.
- A repo-local hosted backend. Durable product data is local filesystem state plus third-party app state.
- Adjacent extension repositories. WebSocket/deep-link contracts are implemented here, but the extension code is not present in this repo.

## Local Project Paths

- `package.json`: npm scripts, frontend dependencies, Tauri CLI entry, app version.
- `vite.config.ts`: Vite/Tauri dev server config, `TAURI_DEV_HOST`, fixed dev port `1420`, manual chunks.
- `src/main.tsx`: i18n initialization, React mount, and `AppRuntimeGuard`.
- `src/App.tsx`: page state machine, lazy page loading, Tauri event bridge, updater, external import handling, close behavior, and direct `FloatingCardWindow` rendering for `floating-card` / `instance-floating-card-*` windows.
- `src/types/navigation.ts`: page enum. There is no React Router.
- `src/types/platform.ts`: platform ID list and platform-to-page mapping.
- `src/services/*.ts`: frontend Tauri `invoke()` wrappers.
- `src/stores/*.ts`: Zustand stores and localStorage caches.
- `src-tauri/src/lib.rs`: Tauri builder, plugins, setup tasks, window events, centralized `generate_handler!` command registry.
- `src-tauri/src/commands/*.rs`: Tauri command wrappers.
- `src-tauri/src/modules/*.rs`: backend behavior, persistence, vendor integration, local services, process management.
- `src-tauri/tauri.conf.json`: app windows, bundle resources, deep-link schemes, updater endpoint/public key.
- `src-tauri/capabilities/*.json`: frontend plugin permissions for `main`, `floating-card`, and `instance-floating-card-*` windows, including floating-card window resize permissions.
- `crates/cockpit-core`: shared Rust logic subset used by the CLI; the Tauri app runtime truth is `src-tauri`.
- `crates/cockpit-cli`: small CLI for selected provider account listing/switching.
- `scripts/release/*` and `Casks/cockpit-tools.rb`: release checks, GitHub Release/Cask helper scripts, updater manifest helpers, checksums.

## Runtime Sources Of Truth

- UI route state: `src/App.tsx` owns `page: Page` and renders pages directly.
- Main navigation: `src/components/layout/SideNav.tsx`, `src/stores/usePlatformLayoutStore.ts`, and `src/types/platform.ts`; the default `codebuddy-suite` group contains CodeBuddy, CodeBuddy CN, and WorkBuddy.
- Frontend i18n: `src/i18n/index.ts`; `en` and `zh-cn` are boot resources, other locale JSON files are lazy-loaded. `localStorage(app-language)` stores the UI language choice.
- Frontend provider caches: Zustand/localStorage caches improve perceived startup. Backend files and third-party app state remain the durable truth.
- Tauri command registry: `src-tauri/src/lib.rs` is the registration source for commands callable from the frontend.
- User settings: `~/.antigravity_cockpit/config.json`, loaded into `src-tauri/src/modules/config.rs` runtime memory.
- WebSocket server status: `~/.antigravity_cockpit/server.json`.
- Antigravity accounts: `~/.antigravity_cockpit/accounts.json` plus `~/.antigravity_cockpit/accounts/*.json`.
- Provider accounts: `~/.antigravity_cockpit/<provider>_accounts.json` plus `~/.antigravity_cockpit/<provider>_accounts/*.json` for provider modules such as Windsurf, Kiro, Cursor, Gemini, GitHub Copilot, CodeBuddy, CodeBuddy CN, Qoder, Trae, WorkBuddy, and Zed.
- Provider selected account state: `~/.antigravity_cockpit/provider_current_accounts.json` plus provider-specific local app state resolvers.
- Instance stores: `~/.antigravity_cockpit/*_instances.json`, with Antigravity using `instances.json`.
- Layout/group state: `group_settings.json`, `account_groups.json`, `codex_account_groups.json`, `codex_model_providers.json`, `tray_layout.json`, `zed_runtime.json`, and frontend layout localStorage.
- Wakeup state: `wakeup_tasks.json`, `wakeup_history.json`, `wakeup_verification_state.json`, Codex `codex_wakeup_tasks.json`, `codex_wakeup_history.json`, `codex_wakeup_runtime_config.json`, and managed `codex_wakeup_homes/`.
- OAuth pending state: `~/.antigravity_cockpit/oauth_pending/*.json` plus provider-specific pending files such as `codex_oauth_pending.json`, `windsurf_oauth_pending.json`, `kiro_oauth_pending.json`, `trae_oauth_pending.json`, `gemini_oauth_pending.json`, and `zed_oauth_pending.json`.
- Codex local access: `codex_local_access.json` and `codex_local_access_stats.json`.
- Antigravity quota cache: `cache/quota_api_v1_desktop/<source>/<hashed-email>.json`.
- Logs: `~/.antigravity_cockpit/logs/app.log*` and `codex-api.log*`.
- Backups: `~/.antigravity_cockpit/backups`.
- Announcement cache/read state: `announcement_cache.json`, `announcement_read_ids.json`, and debug-only `announcements.local.json`.
- Update state exception: `dirs::data_local_dir()/cockpit-tools/update_settings.json` and `pending_update_notes.json`.
- Version sync: `scripts/sync-version.js` copies `package.json.version` into `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`.

## Third-Party Local State Boundaries

- Antigravity token injection reads/writes `state.vscdb` under platform-specific Antigravity app data.
- Codex reads/writes `CODEX_HOME` or `~/.codex`, especially `auth.json` and `config.toml`; Codex session management also touches instance `state_5.sqlite` and `session_index.jsonl`, with deleted session material under `~/.Trash/cockpit-tools-codex-session-trash`.
- Gemini CLI reads/writes `~/.gemini` files such as `oauth_creds.json`, `google_accounts.json`, `settings.json`, and macOS keychain-backed credentials.
- Qoder, Cursor, Windsurf, Kiro, CodeBuddy, CodeBuddy CN, Trae, GitHub Copilot, WorkBuddy, and Zed modules read or write vendor app storage such as `state.vscdb`, `storage.json`, local token files, and app profile directories.
- OpenCode/OpenClaw integration writes local auth projection files when enabled by settings.
- `src-tauri/capabilities/*.json` constrains frontend plugin APIs; Rust modules perform the real filesystem and process operations.

## End-To-End Flows

- App startup: `src-tauri/src/main.rs` calls `antigravity_cockpit_tools_lib::run()`, `src-tauri/src/lib.rs` initializes logging, config/proxy env, Tauri plugins, background services, OAuth restore tasks, token keeper, wakeup schedulers, tray, floating-card startup, deep links, and the command registry. The frontend mounts through `src/main.tsx`.
- UI request path: page/component -> `src/services/*` or store action -> `invoke()` -> `src-tauri/src/commands/*` -> `src-tauri/src/modules/*` -> local JSON/vendor files/process/network -> returned DTO -> Zustand/local component state.
- Account import/switch path: provider page/hook calls provider service, Rust module updates account index/detail files, writes provider local app state where required, refreshes current account state, emits WebSocket/Tauri/tray refresh signals where implemented.
- Instance path: `src/services/platform/createPlatformInstanceService.ts` maps a command prefix to `*_get_instance_defaults`, `*_list_instances`, `*_create_instance`, `*_start_instance`, `*_stop_instance`, and related commands; Rust modules persist `*_instances.json` and call `process.rs` helpers to launch or stop app profiles.
- Wakeup path: Antigravity wakeup pages call `wakeup_*` commands, which use `wakeup.rs`, `wakeup_gateway.rs`, `wakeup_scheduler.rs`, and `wakeup_history.rs`. Codex wakeup uses `codex_wakeup_*` commands and its own runtime config/tasks/history files.
- External import path: deep-link schemes `cockpit-tools://` and `cockpittools://`, single-instance args, and startup args flow into `modules::external_import`; frontend consumes pending import events through `external_import_take_pending` and `external_import_fetch_import_url`.
- Data transfer path: frontend `dataTransferService.ts` exports accounts/config with account references, calls `data_transfer_*` commands for user config and instance stores, and sanitizes instance process PIDs on import.

## Local Services

- WebSocket service: `src-tauri/src/modules/websocket.rs` starts during Tauri setup, binds `0.0.0.0:<ws_port..ws_port+99>`, accepts loopback clients and Windows WSL /16 prefixes, writes `server.json`, and handles extension data sync plus plugin switch responses. `ws_enabled` is persisted and participates in settings restart decisions, but `websocket::start_server()` does not gate startup on `ws_enabled`.
- Web report service: `src-tauri/src/modules/web_report.rs` starts when `report_enabled` is true, binds `0.0.0.0:<report_port..report_port+99>`, serves `GET /report?token=...`, supports markdown/yaml/html output, and can trigger auth refresh checks on access.
- Codex local access: `src-tauri/src/modules/codex_local_access.rs` stores a generated API key and account collection, binds `0.0.0.0`, exposes OpenAI-compatible `/v1/chat/completions`, `/v1/responses`, `/v1/images/generations`, `/v1/images/edits`, validates `Authorization: Bearer` or `X-API-Key`, routes to eligible Codex accounts, and records usage stats.
- OAuth listeners: generic Google OAuth uses random loopback callback ports in `oauth_server.rs`; Codex uses `127.0.0.1:1455` and `/auth/callback`; other provider OAuth modules maintain their own listener/pending-state patterns.

## Infrastructure Snapshot

- App hosting: local Tauri desktop bundle.
- Frontend dev server: Vite on port `1420`, strict port.
- API hosting: no repo-owned remote API verified.
- Update endpoint: `https://github.com/jlcodes99/cockpit-tools/releases/latest/download/latest.json`.
- Announcement endpoint: `https://raw.githubusercontent.com/jlcodes99/cockpit-tools/main/announcements.json`.
- Release target helpers: GitHub Releases, Homebrew Cask, Tauri updater artifacts for macOS, Windows, and Linux, local `release-artifacts`.
- Database: local JSON files and third-party app SQLite/state files; no repo-owned SQL service.
- CDN/signed asset layer: GitHub release assets and updater signatures.
- Firewall/WAF: none in repo; local service exposure is controlled by bind address, token/API key checks, and client allowlists.

## Environment Contract

- Required env vars for startup: none verified.
- Frontend dev: `TAURI_DEV_HOST` controls Vite host and HMR host.
- Codex paths: `CODEX_HOME`, `CODEX_CLI_PATH`.
- OpenClaw paths: `OPENCLAW_CLI_PATH`, `OPENCLAW_REPO_DIR`, `OPENCLAW_STATE_DIR`, `CLAWDBOT_STATE_DIR`, `OPENCLAW_AGENT_DIR`, `PI_CODING_AGENT_DIR`.
- Antigravity quota/wakeup overrides: `ANTIGRAVITY_CLOUD_CODE_URL_OVERRIDE`, `ANTIGRAVITY_APP_QUALITY`, `ANTIGRAVITY_IS_GOOGLE_INTERNAL`, `AG_LOAD_CODE_ASSIST_NODE_VERSION`, `AG_WAKEUP_TRANSPORT_MODE`, `AG_WAKEUP_GATEWAY_BASE_URL`, `AG_WAKEUP_OFFICIAL_LS_BINARY_PATH`, `AG_WAKEUP_OFFICIAL_LS_APP_DATA_DIR`, `AG_WAKEUP_OFFICIAL_APP_VERSION`.
- Process diagnostics: `AG_STRICT_PROCESS_DETECT`, `COCKPIT_COMMAND_TRACE`, `COCKPIT_CHILD_LOGS`, `COCKPIT_CHILD_DETACH`.
- OS path discovery: `APPDATA`, `LOCALAPPDATA`, `PROGRAMFILES`, `PROGRAMFILES(X86)`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `APPIMAGE`, `LANG`, `HOME`, and `PATH`.
- Runtime proxy config: `config.global_proxy_enabled`, `global_proxy_url`, and `global_proxy_no_proxy` rewrite process proxy env keys `http_proxy`, `https_proxy`, `HTTP_PROXY`, `HTTPS_PROXY`, `all_proxy`, `ALL_PROXY`, `no_proxy`, and `NO_PROXY`.
- Linux WebKit: startup sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` when it is absent.

## Command Registration Checks

- Add Tauri commands in both `src-tauri/src/commands/<area>.rs` and the central `tauri::generate_handler!` list in `src-tauri/src/lib.rs`.
- The frontend references `wakeup_cancel_scope` and `wakeup_release_scope`; Rust command functions exist in `src-tauri/src/commands/wakeup.rs`, and the central handler list in `src-tauri/src/lib.rs` does not register them.

## Child Codemaps

- No child codemap files exist.
- Add a child codemap when Codex local access, wakeup, provider injection, release/updater, or another subsystem needs details that would crowd this file.

## Read-First Files

- `package.json`
- `vite.config.ts`
- `src/main.tsx`
- `src/App.tsx`
- `src/types/navigation.ts`
- `src/types/platform.ts`
- `src/components/layout/SideNav.tsx`
- `src/hooks/useProviderAccountsPage.ts`
- `src/stores/createProviderAccountStore.ts`
- `src/stores/useAccountStore.ts`
- `src/stores/useCodexAccountStore.ts`
- `src/services/platform/createPlatformInstanceService.ts`
- `src/pages/AccountsPage.tsx`
- `src/pages/CodexAccountsPage.tsx`
- `src/pages/SettingsPage.tsx`
- `src-tauri/src/lib.rs`
- `src-tauri/src/modules/config.rs`
- `src-tauri/src/commands/system.rs`
- `src-tauri/src/modules/account.rs`
- `src-tauri/src/modules/codex_account.rs`
- `src-tauri/src/modules/process.rs`
- `src-tauri/src/modules/websocket.rs`
- `src-tauri/src/modules/web_report.rs`
- `src-tauri/src/modules/codex_local_access.rs`
- `src-tauri/src/modules/update_checker.rs`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/default.json`
- `crates/cockpit-core/src/lib.rs`
- `crates/cockpit-cli/src/main.rs`
