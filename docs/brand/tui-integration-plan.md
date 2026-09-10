# TUI brand integration plan

Scope: bring the Herduck identity (see `assets/brand/README.md`) into the app itself.
This touches `src/`, so it waits until the current Codex branch lands. Everything listed below
reuses existing UI structure (`render_modal_shell`, `render_action_button`, palette tokens);
no new screen types.

## Touchpoints

| # | Where | Change | File |
| --- | --- | --- | --- |
| 1 | `herduck --version` / `--help` header | Print `herduck-terminal-mini.txt` above the version line when stdout is a TTY. Plain text otherwise. | `src/main.rs` (~L712, L722) |
| 2 | Onboarding welcome modal | Replace the `herduck · welcome` text header with the mini mark (8 rows, accent colour) on the left and the existing copy on the right. Widen modal from 64×16 to 72×18. Below 72 cols fall back to the current text header. | `src/ui/onboarding.rs::render_onboarding_welcome` |
| 3 | Empty states | `No sessions yet` in Projects/Sessions/Topics: show the mini mark centred above the message, dimmed (`overlay0`). Only when the pane is ≥ 12 rows and ≥ 30 cols. | `src/ui/projects.rs:209` and the equivalent in sessions/topics |
| 4 | Client attach wait | While the client is connecting to the server (currently blank), render the full mark centred with `Make AI work for you.` and a one-line status. Disappears on first frame from the server. | client attach path in `src/app/` / `src/main.rs` |
| 5 | Status bar | Prefix the left segment with `>_` in `accent`. Two cells, no text change. | `src/ui/status.rs` |

Tagline copy is `Make AI work for you.` everywhere. The description string
`terminal workspace manager for coding agents` in onboarding stays as the second line.

## Assets in the binary

Embed with `include_str!("../../assets/brand/herduck-terminal.txt")` and the mini variant into a
`src/ui/brand.rs` module exposing:

```rust
pub const MARK: &str = ...;
pub const MARK_MINI: &str = ...;
pub const TAGLINE: &str = "Make AI work for you.";
pub fn mark_size(mark: &str) -> (u16, u16); // cols, rows
pub fn render_mark(frame, area, mark, style);  // centres, clips when too small
```

Colour: `palette.accent` (already the yellow token in the default theme). Do not hardcode RGB.

## Tests

- `brand::mark_size` returns the expected dimensions for both marks (guards against accidental
  edits to the txt files breaking layout).
- Onboarding render test at 80×24 asserts the mark's first row appears; at 60×20 asserts the
  text fallback. Prove each test fails first by flipping the width threshold.
- `--version` output with a non-TTY stdout contains no braille/box characters.

## Out of scope

- Animated splash, colour-per-glyph, sound. The mark is one colour, static.
- Changing the theme palette itself.
