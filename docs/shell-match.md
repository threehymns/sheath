# Prompt: hill-climb the boot shell to a pixel-identical render

Copy everything below into an agent session working in this repository. It is self-contained: the
two tools it names live at `scripts/shell-shot` and `scripts/shell-diff`.

---

## Goal

`web/index.html` paints a boot shell while the wasm module downloads. Its job is to be visually
indistinguishable from the app's first frame, so nothing shifts when Rune takes over the canvas.

Make the shell match a reference screenshot of the running app **exactly** — same pixels, not
"close enough". Work by measurement: render, diff, change one thing, re-diff. Never guess a colour,
size or icon: take it from the code or from the reference.

Acceptance:

- `scripts/shell-diff` reports **0 differing pixels** outside text glyph interiors.
- Glyph interiors (antialiasing) may differ by up to `--tolerance 8`; everything else must be 0.
- No layout box, border, radius, icon path, or background colour may differ at all.

## Ground truth

You need one reference PNG per theme, captured from the app itself, at the exact settings the loop
renders with. If `docs/reference/shell-dark.png` does not exist, ask the user for it (or capture it
yourself) with:

- Chrome window **1180×760**, zoom **100%**, device pixel ratio **1**.
- `prefers-color-scheme` matching the file name (`dark` or `light`).
- **Default pane widths**: sidebar 212, inspector 286. A saved `session.kdl` restores whatever the
  user last dragged, which silently breaks the comparison — delete or reset it before capturing,
  or record the widths and pass the same ones to the shell.
- No rows selected, no hover, no focus ring, no caret, no toast, no scrollbar.
- Path `/home/you` in the web demo tree (6 items), inspector open on "No selection".

Save as `docs/reference/shell-<theme>.png`. **Capture the reference in the same browser build on
the same OS that runs the loop**: Chrome's font rasterisation and hinting change between versions
and platforms, so a reference from another machine can never reach 0.

## The loop

```sh
scripts/shell-shot --theme dark --size 1180x760 --out target/shell-dark.png
scripts/shell-diff docs/reference/shell-dark.png target/shell-dark.png \
    --diff target/shell-diff.png --tolerance 0
```

`shell-shot` copies `web/index.html` to `target/shell.html` with every `<script>` removed, so the
page stays on its shell and never boots the app, then screenshots it from `file://` with fixed
flags: headless, GPU off, 1× scale, no scrollbars, no LCD text, no hinting, `--force-dark-mode` for
the dark theme. `--size` sets the viewport exactly.

`shell-diff` decodes both PNGs and prints the differing pixel count, max/mean channel delta, the
bounding box of the differences, and the worst pixels. `--diff` writes a magenta-on-dim image:
open it, and a 1-pixel offset or a wrong radius is immediately visible. `--region X,Y,W,H` compares
a rectangle only; `--tolerance N` ignores deltas up to N.

Iterate:

1. Run both commands.
2. Read the bbox. It tells you *what* moved: a full-width band means geometry, a single row means
   one control, scattered pixels mean colour or antialiasing.
3. Change **one** value in `web/index.html`.
4. Re-run. Keep the change only if the differing count goes down; otherwise revert it.
5. Repeat until the count is 0. Then run the light theme, and once more at a second size
   (e.g. `--size 1440x900`) to catch values that were tuned to one width.

Record each attempt in a scratch table (`attempt | change | differing px | max delta`) so a
regression is obvious and a dead end is not retried.

## Order of attack

Cheap and global first, because each step changes the diff basin for the next:

1. **Structure** — the shell must have the same boxes: sidebar, toolbar band above the card, card,
   inspector, path bar, table header, rows. A missing or nested-wrong container makes every other
   measurement meaningless (`shell-diff` will show a large bbox).
2. **Colours** — backgrounds, borders, text and icon inks. One wrong token shifts thousands of
   pixels. Take values from `themes/opencode/assets/theme.json` (the `v2-…` keys) and the kit's
   `…-base` keys, not from a colour picker on a compressed screenshot.
3. **Geometry** — pane widths, frame insets, band heights, row heights, column widths, paddings,
   gaps, radii. Compare the bbox edges against the reference: `bbox x 0..211` means the sidebar is
   212 wide, and the diff on its own tells you which side of the boundary is wrong.
4. **Typography** — font family, size, weight, colour, letter spacing. Text will not reach 0 delta
   against the canvas renderer because Rune rasterises glyphs itself; match the metrics (baseline
   and cell edges) and accept AA differences inside glyphs. Use `--region` around one label while
   tuning its position.
5. **Icons** — replace any approximation with the exact `Icon` body and `view` from
   `src/icons.rs`, `rune_kit::glyphs` or `rune_theme_opencode::icons`, at the size the control
   draws it (`IconButton` sizes, `icon(…, N)` calls). A wrong path is visible as a hazy cluster in
   the diff image; a wrong stroke width as a delta along every edge.
6. **States** — selected row fill, zebra parity (`i % 2 == 1` stripes the *second* row), the
   inspector grip, the search keycap. These are small but they are what people notice.

## What "byte identical" can and cannot mean

- The PNG **file** will not be byte-identical: encoders differ. Compare **decoded pixels**
  (`shell-diff` does), not file hashes.
- The shell is HTML/CSS rendered by Chrome; the app is drawn into a WebGPU canvas by Rune. Text
  antialiasing will differ (subpixel position, hinting, gamma). Aim for identical geometry and
  colour, and near-identical glyph rendering. If a text region is the only thing left, measure it
  with `--region` and `--tolerance`.
- Fonts must exist on the loop machine: the shell asks for `Inter` but falls back to `system-ui`.
  If the reference uses Inter, the glyphs will not match — either ship the face or accept the
  difference and say so.

## Rules

- Edit only `web/index.html` (plus `docs/` for notes). Do not change the app, the theme, or the
  build to win a diff.
- Keep the page's behaviour intact: the module script that boots the app, the early wasm fetch, the
  OPFS hydration call, and the error path. `shell-shot` strips scripts at render time; do not strip
  them from the source.
- Every value you set must trace to a source: a theme token, a kit constant, or a measurement from
  the diff. Add a comment the first time a magic number appears.
- Rebuild with `scripts/web` after edits, so the bundle in `target/web` carries the same page.
- Report the final numbers: differing pixels per theme and size, and any region left with a
  documented tolerance.

## Report format

```
dark 1180x760: 0 px differ (tolerance 0)
light 1180x760: 0 px differ (tolerance 0)
dark 1440x900: 0 px differ (tolerance 0)
text: max delta 6 inside glyphs, 0 outside
```
