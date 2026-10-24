# Parler design system

Parler should feel obvious to someone who has never heard of an API key, and
calm enough to live on screen all day. Everything below serves that.

The tokens live in [`src/styles.css`](../src/styles.css); the components in
[`src/ui`](../src/ui). To see them all without building the app, run `pnpm dev`
and open <http://localhost:1420/preview.html> (see
[contributing](contributing.md)).

## Principles

1. **Show, don't explain.** A moving example beats a paragraph: the onboarding
   scenes type a message, press the keys, flip the switches.
2. **One thing per screen.** Each onboarding step asks for one decision; each
   settings card covers one subject.
3. **Plain words.** "Your key", not "credentials"; "Type it", not "Paste
   mode". Technical options fold away under *Advanced options*.
4. **Calm until you speak.** The interface is paper and ink. Vermilion, the
   recording light, means *Parler is listening* (and marks the brand). It is
   never decoration.
5. **Every problem comes with its fix.** An error names what happened in a
   sentence and offers the button that solves it.

## Tokens

### Colour

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `canvas` | `#f6f4ef` | `#141312` | Window background, with a faint paper grain |
| `sidebar` | `#efece5` | `#191816` | Sidebar, onboarding stage |
| `surface` | `#ffffff` | `#1f1d1b` | Cards |
| `surface-2` | `#f7f5f0` | `#262421` | Fields, hovered rows, chips |
| `text` / `text-2` / `text-3` | `#1a1815` / `#57524a` / `#736d63` | `#f3f0ea` / `#bdb7ac` / `#8f897d` | Primary, secondary, tertiary text |
| `primary` | ink | paper | Primary buttons |
| `accent` | `#e8452c` | `#ff5a3c` | Recording, errors, brand mark (fills and icons) |
| `accent-text` | `#c4371f` | `#ff8a72` | Vermilion text and links, dark enough to read |
| `success` | `#25774a` | `#5fc48a` | Done, allowed, switches on |
| `warning` | `#b7791f` | `#e0a94a` | Working, tips |
| `ink` / `on-ink` | `#1e1c19` / `#f6f2ea` | `#0c0b0a` / `#f6f2ea` | Hero card, key card, the dictation pill |

**Tints** colour the icon tiles; each area keeps its own so people learn where
they are: transcription violet, shortcut amber, microphone red, text blue,
general stone. Services: Groq amber, OpenAI green, Fireworks red, custom blue.
Services are shown with an icon and tint, never a borrowed logo.

### Type

| Role | Font | Size |
| --- | --- | --- |
| Hero (welcome) | Instrument Serif | 44 px (`text-hero`) |
| Page and step titles | Instrument Serif | 32 px (`text-title`) |
| Section titles | Instrument Serif | 22 px (`text-heading`) |
| Interface | Schibsted Grotesk | 14 px body, 13 px hints, 12 px captions |
| Keys, models, times | IBM Plex Mono | 10.5–13 px |

Serif titles may put their second half in *italics* ("Your voice, *typed
anywhere*."). Fonts are bundled, so Parler looks the same offline.

**Theme**: light and dark follow the system by default; *Settings → General
→ Appearance* can force either (`data-theme` on the root element, which the
tokens and Tailwind's `dark:` variant both honour).

### Shape, depth, motion

- Radius: fields 12 px, cards 18 px (`rounded-card`), hero and stage 22–26 px,
  buttons and pills fully round.
- Depth: `shadow-card` is a hairline; `shadow-raised` lifts the hero, the
  onboarding scenes and anything floating.
- Motion: entrances rise 10 px and fade in over 0.55 s (`rise`), staggered by
  40–60 ms. The voice bars `dance`, the caret blinks, connectors `flow`.
  Everything stops under *Reduce motion*.

## Components (`src/ui`)

| Component | Use |
| --- | --- |
| `LogoMark`, `Wordmark` | The brand. `LogoMark live` moves its bars while listening. |
| `Button` | `primary` for the one main action of a screen, `secondary` otherwise, `ghost` for back and skip, `accent` sparingly. Sizes `sm`, `md`, `lg` (onboarding). |
| `IconButton` | Icon-only actions; always labelled. |
| `Card` | Every group of content. |
| `IconTile` | An icon on its tint, at the head of a section, choice or stat. |
| `Keycaps` | Shortcuts, drawn as physical keys; `pressed` pushes them down, `dark` for ink surfaces. |
| `FlowPill` | The dictation pill as drawn inside the app (`idle`, `listening`, `writing`). |
| `Banner` | A message that needs the user, with its actions (`error`, `warning`, `success`, `info`). |
| `StatusDot` | Ready, listening, busy, needs attention. |
| `Stat` | A number worth being proud of. |
| `Chip` | A removable word (vocabulary). |
| `StepDots` | Progress through the onboarding. |
| `PageHeader`, `SectionHeader` | The title of a page; the head of a settings card. |
| `Field`, `Input`, `TextArea`, `Select`, `Toggle`, `Segmented` | Forms. `Field inline` puts the control on the right. |
| `ChoiceCard` | A big radio option with icon, description and optional badge. |
| `Disclosure` | Folds away what most people never need. |

## Layouts

- **Application**: a 236 px sidebar (brand, navigation: Home, History,
  Vocabulary, Settings; a status card showing state, service and shortcut)
  and a centred column up to 700 px wide.
- **Onboarding**: words on the left (title, one decision, the main action at
  the bottom right), a *stage* on the right where the scene plays.
- **The pill**: a dark capsule centred at the bottom of the screen, never
  focusable: a vermilion dot and live bars while listening, a spinner and a
  wave while writing, a few words when something went wrong.

## Writing

- Speak to the person: "Hold ⌥ Space and speak", "Your words will appear
  here". French uses *vous*.
- Buttons are verbs: *Continue*, *Try again*, *Open settings*.
- Errors say what happened and what to do, in one or two sentences, without
  codes unless the service sent one.
- Every string exists in English and French (`src/i18n/locales`); a test fails
  when they drift apart.

## Accessibility

- Text meets WCAG AA contrast (4.5:1) on its surfaces in both themes; that is
  why vermilion text uses `accent-text` rather than `accent`.
- Focus is visible everywhere (a vermilion ring; fields use their own).
- Click targets are at least 32 px; icon-only buttons carry labels.
- Animations respect *Reduce motion*.
