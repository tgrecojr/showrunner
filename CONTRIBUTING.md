# Contributing to Showrunner

Thanks for taking an interest. Showrunner is a small, opinionated, self-hosted app, and contributions are welcome.

## Open an issue first

**Please open an issue before you open a pull request.** This applies to bug fixes, features, and refactors alike.

The reason isn't bureaucracy — it's that a short conversation up front saves you from writing code that gets turned down for a reason you had no way to know about (it's already in progress, it conflicts with a planned change, or it's deliberately out of scope). A one-paragraph issue costs you five minutes; a rejected PR costs you an afternoon.

There are two issue templates:

- **Bug report** — something is broken. Include your setup (Docker or local dev), the relevant `docker compose logs app` output, and what you expected instead.
- **Feature request** — describe the problem you're trying to solve, not just the solution you have in mind. Often there's an easier path.

Once we've agreed on the shape of a change in the issue, open the PR and link it.

## What's in scope

Showrunner is a personal TV tracker for a homelab. Things that fit well:

- Bug fixes
- New notification channels (the `Notifier` trait is designed for exactly this — see `backend/src/notifications/`)
- Better TMDB data handling, more accurate air dates, edge cases in season/episode structures
- UI/UX improvements to the existing pages
- Tests, docs, accessibility

Things that likely need discussion first:

- New external data sources beyond TMDB
- Multi-user support, authentication, and authorization (this is a known future design topic, not a quick add — see [Security model](README.md#security-model))
- Anything that changes the deployment story away from "one container, one SQLite file"

## Development setup

See [Local development](README.md#local-development) in the README to get the backend and frontend running.

## Before you push

Run the full gate locally. CI runs exactly these, and it's faster to fix them on your machine than to iterate through a red build.

**Backend:**

```bash
cd backend
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo llvm-cov --lcov --output-path lcov.info --fail-under-lines 85
```

**Frontend:**

```bash
cd frontend
npx tsc --noEmit
npm run lint
npm run test:coverage
```

Notes on the gate:

- Clippy runs with `-D warnings`. A warning fails the build.
- Backend line coverage must stay at or above **85%**. If you add code, add tests.
- `cargo fmt` and ESLint are not suggestions — formatting differences will fail CI.

## Pull request expectations

- **Link the issue** it resolves (`Closes #123`).
- **One logical change per PR.** A bug fix and a refactor in the same PR are hard to review and hard to revert.
- **Keep the commit history readable.** Conventional-commit style (`feat:`, `fix:`, `chore:`, `docs:`) is used throughout this repo; please match it.
- **Update the docs** when you change behavior — the README config table and `CLAUDE.md` architecture notes should never lie about the code.
- **Don't commit secrets.** No API keys, no webhook URLs, no `.env` files, no database files. The `.gitignore` covers the usual cases, but check `git diff --cached` before you commit.

## Adding a notification channel

This is the most likely contribution, so here's the shape of it:

1. Add a file in `backend/src/notifications/` implementing the `Notifier` trait.
2. Register it in the dispatcher (one line).
3. Add its config to `Config::from_env` in `backend/src/config.rs` — the channel should be disabled when its env var is blank, matching how `SLACK_WEBHOOK_URL` works.
4. Add it to `env.example` and the README config table.
5. Add tests. The existing Slack tests use `wiremock` to stub the webhook endpoint; follow that pattern.

The dedupe layer (`notification_log`) is channel-aware and handled centrally, so you don't need to reimplement it.

## Reporting a security issue

Please **don't** open a public issue for a security vulnerability. See [SECURITY.md](SECURITY.md) for how to report it privately.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE) that covers this project.
