<!--
Thanks for contributing! Please make sure an issue exists and has been
discussed before opening this PR — see CONTRIBUTING.md.
-->

## Related issue

Closes #

## What does this change?

<!-- A short description of the change and why it's the right approach. -->

## How was it tested?

<!-- What did you run, and what did you observe? "CI is green" is not a test plan. -->

## Checklist

- [ ] This change was discussed in a linked issue first
- [ ] `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass
- [ ] `cargo llvm-cov --fail-under-lines 85` passes (backend coverage held or improved)
- [ ] `npx tsc --noEmit`, `npm run lint`, and `npm run test:coverage` pass
- [ ] Docs updated (README config table / `CLAUDE.md` / `env.example`) if behavior changed
- [ ] No secrets, API keys, webhook URLs, `.env` files, or `.db` files in the diff
