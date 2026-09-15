# Herduck brand assets

The white duck has uneven black glasses and a golden bill. The README banner and
application icon use the supplied full-color artwork; terminal marks preserve the
same silhouette with single-cell Unicode Braille characters.

- `herduck-color.png`: full-color portrait, cropped from the approved artwork.
- `../herduck-logo.png`: README banner with the Herduck wordmark and tagline.
- `../herduck-icon.png`: square color icon.
- `herduck-16-braille.txt`: 8 columns × 4 terminal rows, for the menu and compact welcome.
- `herduck-32-braille.txt`: 16 columns × 8 terminal rows, for the welcome and empty workspace.

Keep the text marks in a monospace font, preserving whitespace and line breaks.
The terminal renderer uses the current theme's foreground for the outline and its
yellow accent for the wordmark. Render branding in the client UI, outside Agent PTYs.
