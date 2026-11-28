# T19 — Resizable columns

*Pulled by: theoria's own `Knobs · struct` story (2026-11-28), whose side
panel holds a struct's controls, its value as code, and the story's source.
At 280px the code blocks scroll sideways and the controls run long, and the
stage, which shows one small card, keeps the rest of the window.*

## Story

As a Leptos developer working in the gallery, I want to drag the divider
between the stage and a side column to change how wide each is, so that I
can give the panel room when I am editing args or reading code, and give it
back to the stage when I am looking at the component.

## Vertical slices

- **T19.0 — drag the panel's divider** ✅
  - [x] a divider between the stage and the side panel, dragged with the
        pointer, sets the panel's width
  - [x] the width stays between a least and a most (the stage never
        disappears, the panel never collapses below its controls' minimum)
  - [x] the divider is a focusable `role="separator"` with its orientation
        and current value, and the arrow keys move it by a step
  - [x] the width is kept in `localStorage` across a reload, as the
        selected story is
  - [x] a double-click on the divider puts the default width back
- **T19.1 — drag the sidebar's divider**
  - [ ] the same, between the sidebar and the stage, kept under its own
        key

## Notes / refs

- Ruled 2026-11-28: both dividers, the panel's first since it is what ran
  out of room; each width kept in `localStorage`, as Storybook does and as
  the selected story is, so Trunk's hot reload on every save keeps it; a
  double-click puts the default back, as in Storybook and VS Code.
- The decisions are plain functions a native test can call: the width a
  drag gives (the pointer's position, clamped to the bounds) and the width a
  key gives (a step either way, clamped). The pointer and key handlers only
  read the event and call them, and are left to theoria-e2e, as the control
  handlers are.
- Built in T19.0: the panel defaults to 280px and goes no narrower than
  220px, the stage keeps 200px, and a key moves the divider 16px. Its most
  (`aria-valuemax`) is measured from the drawn stage and panel when the
  panel is drawn and when the divider is focused, pressed, or keyed, not
  on a window resize, since Leptos's window listener has no no-op off the
  browser and would panic under leptotes. A `ResizeObserver` attached from
  `NodeRef::on_load` would keep it current on a resize without that panic,
  and could serve both dividers once T19.1 adds the sidebar's. A window
  too narrow for a stored width shrinks the panel, down to its
  `min-width`, rather than the stage.
- The panel still scrolls when its content is taller than the window
  (T18.4); a wider panel shortens the code blocks' sideways scroll, not the
  panel's own.
- Not pulled: a divider below the stage (Storybook's addon panel at the
  bottom), or collapsing a column to nothing.
