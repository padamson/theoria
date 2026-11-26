# T14 — A URL for each story

*Pulled by: pyle, whose browser tests click a story by its sidebar label, and
two of its stories share the label `Playground` (`Theme/Playground` and
`Version bar/Playground`).*

## Story

As a Leptos developer, I want each story to have its own URL, so that a link
or a bookmark opens that story and a browser test can open a story by its
full name.

## Vertical slices

- **T14.0 — a story's URL** ✅
  - [x] `?story=<full name>`, form-encoded but with each `/` left as it is
        (`Version bar/Playground` is `?story=Version+bar/Playground`), opens
        that story, whatever the stored selection is
  - [x] with no `story` in the URL, the gallery opens the stored story, and
        the first story when nothing stored names one
  - [x] a URL naming no story says so on the stage and marks nothing in the
        sidebar
  - [x] selecting a story rewrites the URL to name it, without a history
        entry, so a reload or a copied URL keeps the story
  - [x] each sidebar leaf is a link to its story's URL; a click with a
        modifier key is left to the browser (a new tab or window) and does
        not change the selection
  - [x] a URL opens its story when another story shares its last segment
  - [x] theoria-e2e opens every story by its URL
- **T14.1 — the e2e suite opens stories by URL**
  - [ ] every other theoria-e2e test opens its story by URL and switches
        by the sidebar link whose `href` names it, never by the link's text

## Notes / refs

- Ruled 2026-11-25: the URL wins over `localStorage`, and a bare URL falls
  back to it; selecting rewrites the URL with `history.replaceState`, so Back
  leaves the gallery rather than stepping through stories; an unknown name is
  reported, not replaced with another story; sidebar leaves are `<a>` links,
  as in Storybook.
- The query is read and written with `form_urlencoded`, so a name with `&`,
  `#`, `+`, or `%` survives the trip. The gallery writes each `/` raw, which
  a query may hold (RFC 3986), so a nested story's URL reads like its name;
  a space stays `+`. Ruled 2026-11-26 over `%2F` (noisy on every nested
  story), `%20` (no stock Rust encoder writes it, and `+` is the usual query
  space), and a substitute such as `-` (lossy, so a test could not build it
  from the name). A test builds the URL with
  `form_urlencoded::byte_serialize`, which the gallery reads as it is, and
  adds `.replace("%2F", "/")` to match the gallery's `href`s and address
  exactly. `%20` and a raw `/` read the same as `+` and `%2F`.
- `localStorage` still records the selection, for the bare URL `theoria
  serve` opens.
