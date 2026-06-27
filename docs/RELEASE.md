# Release Gate

CodeScribe is releasable only when public surfaces tell the same truth as the code and the macOS artifact is actually usable.

Current source version: `0.12.2`

## Required Before Public Announcement

- Repository description matches the product:
  `Native macOS tray dictation and assistive voice overlay with local Whisper live preview.`
- Repository homepage points to `https://vetcoders.github.io/CodeScribe/`.
- README install section keeps source install as the guaranteed path until current DMGs are verified.
- `CHANGELOG.md` has a current `0.12.x` section.
- GitHub license display is checked against the active `FSL-1.1-ALv2` license.
- The release workflow on `main` builds signed and notarized DMGs, not ad-hoc artifacts.
- Required GitHub secrets are configured:
  - `CODESIGN_CERTIFICATE_BASE64`
  - `CODESIGN_CERTIFICATE_PASSWORD`
  - `CODESCRIBE_CODESIGN_IDENTITY`
  - `APPLE_ID`
  - `APPLE_TEAM_ID`
  - `APPLE_APP_SPECIFIC_PASSWORD`
- Optional `CODESCRIBE_BUNDLE_ID` is either set or the workflow default is accepted.

## Artifact Gate

Both variants must be produced and smoke-tested:

- `CodeScribe_0.12.2.dmg`
- `CodeScribe_0.12.2_full.dmg`

For each DMG:

1. Download from GitHub Releases.
2. Mount on a machine outside the developer environment.
3. Drag `CodeScribe.app` into `/Applications`.
4. Launch without Gatekeeper workaround.
5. Complete onboarding.
6. Verify microphone, Accessibility, Input Monitoring, and Screen Recording prompts.
7. Verify `codescribe --version` reports `0.12.2`.

## Commands

```bash
make check
make release-dmgs
```

## Known External Gaps

- The latest observed live GitHub release may lag the source version.
- GitHub license detection may require manual wording in release notes.
- Signing/notary secrets must be verified before tagging.
- The landing page must not promise a current DMG until the signed DMG exists and passes the artifact gate.
