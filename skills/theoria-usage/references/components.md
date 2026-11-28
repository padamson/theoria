# Building a component with its stories

Each claim about a component goes in the cheapest test that can hold it: a
unit test before a leptotes test, and a leptotes test before a browser
test. Stories are not tests. They are for a person looking at the component.

## The steps, per component

1. Put what the component decides (which state it is in, what text it
   shows, what it orders or hides) in a plain function, and unit-test it
   before writing any markup.
2. Write leptotes tests for the first render. The leptotes skill covers
   them.
3. Write the component and its CSS until those tests pass.
4. Write the stories. Give each state a visitor can arrive at its own story,
   and add a playground with one control per input. Pick each control by
   its parameter's type (`references/usage.md`). Test a playground's
   wiring (which arg feeds which input) with leptotes, through
   `story.view()`, in a build that compiles the stories
   (`references/usage.md`).
5. Hand the gallery to a person. A session can run `theoria serve` but
   cannot see the result, so this step ends by naming the stories to open
   and what a pass looks like in each.
6. Test what only a browser decides (the cascade, a fetch, a navigation,
   a control reaching the stage) with playwright-rust against a real
   page. Two kinds of test may use the gallery's stories, each opening a
   story by its URL:
   - One sweep over every story. It finds the stories in the gallery's
     sidebar at run time, so a new story is covered without touching the
     test, and checks that each opens and renders. A per-story
     accessibility check or screenshot comparison goes in the same loop.
   - A targeted test on a story, when the state it checks is one a person
     would look at in that story (a playground's control changing what
     the stage shows, say).

   A case no visitor would look at (a layer-order case, a stubbed fetch)
   goes on a static page the test serves.

## Mistakes these steps catch

- **A story written for a test.** A story that exists only for a browser
  test to drive (a layer-order case, a stubbed fetch) shows a person
  nothing. Put that case on a static page the test serves. A test that
  opens a story a person would look at is fine (step 6).
- **A story that navigates away.** A link or a menu inside a story opens
  an address the gallery does not serve. Stub the navigation in every
  interactive story. A story of a composed page inherits the problem from
  its parts, so check those stories too.
- **The gallery has no CSS reset, and the site does.** A component can look
  right in the gallery and lose an underline or change width under a
  site's reset (Tailwind's, say). Make the component's rules set whatever a
  reset would change, and check it with a browser test on a page that
  loads the reset.
- **A warm build hides a cold-build failure.** Deep view types in stories
  can overflow rustc's query depth on a cold build only, and CI builds
  cold. Run `cargo test --doc --all-features` from a clean target directory
  before the commit.
- **A site's theme in the library's gallery.** A component library's
  gallery shows its default theme and a theme playground. A site that uses
  the library shows the components under its own theme, in its own
  gallery.
