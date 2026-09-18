# Herduck brand assets

The white duck has uneven black glasses and a golden bill. The README banner and
application icon use the supplied full-color artwork. The sidebar displays a real
32×32 RGBA PNG through the Kitty graphics protocol; welcome screens use text art.

- `herduck-color.png`: full-color portrait, cropped from the approved artwork.
- `../herduck-logo.png`: README banner with the Herduck wordmark and tagline.
- `../herduck-icon.png`: square color icon.
- `herduck-kitty-32-outline.png`: the approved 32×32 transparent bitmap with a dark outline, for the sidebar menu.
- `herduck-16-blocks.txt`: legacy text asset, no longer used by the sidebar.
- `herduck-16-braille.txt`: 8 columns × 4 terminal rows, retained as a compact source asset.
- `herduck-32-braille.txt`: 16 columns × 8 terminal rows, for the welcome and empty workspace.

Keep the text marks in a monospace font, preserving whitespace and line breaks.
The 16px and 32px marks are independently drawn size variants from the approved
logo kit. Use their original grids; do not resample a larger mark or fill its outline.
The sidebar reserves 4 columns × 2 rows for the bitmap, with two columns of left
inset and one row below. Kitty scales the 32px source to that cell area. Enable
`[experimental] kitty_graphics = true` and reconnect the client in a compatible
terminal such as Ghostty. Text-only clients retain the wordmark and menu; narrow
or short sidebars use a single text row. The bitmap stays visible while the menu
above it is open. It is deleted for covering overlays, when the sidebar disappears,
and on disconnect. It never enters a pane
PTY or agent scrollback. The asset is copied unchanged from the approved Pi
logo kit (`terminal/kitty/herduck-kitty-32-outline.png`).
The terminal renderer uses the current theme's foreground for the outline and its
yellow accent for the wordmark. Render branding in the client UI, outside Agent PTYs.
