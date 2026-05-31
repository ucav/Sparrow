# Sparrow Brand Assets

Canonical visual identity for Sparrow. Always use these assets.

## Color Palette

| Token | Hex | Role |
|---|---|---|
| `--bg` | `#0e0b08` | Near-black background |
| `--panel` | `#16120d` | Panel background |
| `--line` | `#2c251c` | Hairline borders |
| `--fg` | `#ece2cf` | Primary text |
| `--dim` | `#897d6c` | Muted text |
| `--dimmer` | `#5c5346` | Faint text |
| `--brand` | `#f2a93c` | Amber — brand, cost |
| `--coral` | `#f0674a` | Coral — secondary accent |
| `--agent` | `#4ec9b0` | Teal — active agent / coder |
| `--planner` | `#6fa6e6` | Blue — routing / planner |
| `--verifier` | `#c9a14e` | Sand — verifier |
| `--add` | `#74c258` | Green — diff + |
| `--rem` | `#d96a63` | Red — diff - |
| `--steel` | `#b9b0a3` | Tool metal |
| `--gold` | `#f2c94c` | Pirate hoop / highlights |

## Typography

**IBM Plex Mono** everywhere. Weights 400/500/600/700.

Wordmark: 700 weight, letter-spacing ~3px, amber→coral gradient.

## Mascot

**Chubby pirate-builder sparrow.** Props:
- Rounded amber body
- Crest tufts
- Open eye + pirate eye patch with strap
- Coral beak
- Pink cheek blush
- Cream belly
- Gold hoop earring
- Wrench held in wing
- Two feet

## Files

| File | Usage |
|---|---|
| `sparrow-logo.html` | Canonical SVG symbols + preview page |
| `sparrow-mascot.svg` | Full mascot (240×240) |
| `sparrow-cockpit.svg` | Compact export of the canonical logo for small UI slots |
| `sparrow-ascii.txt` | Console-native ASCII variant |
| `sparrow-identity.html` | Brand identity page (splash + console concept) |
| `sparrow-presentation.html` | Live demo / concept presentation |

## Usage

**Web/HTML:** Use `<symbol id="sparrow">` from `sparrow-logo.html`.

**Terminal:** Use `sparrow-ascii.txt` verbatim. This is the CLI/TUI rendering of the same pirate-builder mascot when SVG is not available.

**App icon:** Use `sparrow-mascot.svg` or `sparrow-cockpit.svg`.

**WebView and presentation:** Keep the full canonical logo visible in the permanent cockpit bar and reserve the ASCII variant for the terminal transcript area.

## Voice

- Concise, competent, a wink of pirate-builder character
- Never sycophantic, no emoji spam
- Rich personalities live in *agents* (SOUL files), not in base UI

## Tagline

**"one cli · grows with you"**
