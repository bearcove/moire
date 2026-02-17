# Frontend CSS Conventions

CSS is co-located with components. Each component has a `.css` file next to its `.tsx` file, imported from that file.

## Rules

- `styles.css` contains **only**: `:root` design tokens, global resets (`*`, `html`, `body`, `#app`), and `@keyframes`. Do **not** add component styles here.
- Every component's styles live in a `.css` file next to the `.tsx` file.
- To add styles for `Foo.tsx`, put them in `Foo.css` (create it if it doesn't exist) and add `import "./Foo.css";` to `Foo.tsx`.
- Class names use the existing prefix convention — keep the same prefix as the component (`.ui-action-button*`, `.badge*`, `.panel*`, etc.).

## File map

| CSS file | What goes in it |
|---|---|
| `styles.css` | `:root` tokens, `*`/`html`/`body`/`#app` resets, all `@keyframes` |
| `App.css` | App shell, header, snapshot UI, empty state |
| `ui/layout/Panel.css` | `.panel*`, `.panel-header*`, `.panel-collapse*` |
| `ui/layout/Row.css` | `.ui-row*` |
| `ui/layout/Section.css` | `.ui-section*` |
| `ui/layout/SplitLayout.css` | `.ui-split*` |
| `ui/primitives/ActionButton.css` | `.ui-action-button*` |
| `ui/primitives/Badge.css` | `.badge*`, `.waiter-badge*` |
| `ui/primitives/Checkbox.css` | `.ui-checkbox*` |
| `ui/primitives/DurationDisplay.css` | `.duration*` |
| `ui/primitives/FilterMenu.css` | `.ui-filter*` |
| `ui/primitives/SegmentedGroup.css` | `.ui-segmented*` |
| `ui/primitives/Select.css` | `.ui-select*` |
| `ui/primitives/Slider.css` | `.ui-slider*` |
| `ui/primitives/Table.css` | `.ui-table*` |
| `ui/primitives/TextInput.css` | `.ui-input*`, `.ui-text-field*` |
| `components/graph/graph.css` | Graph panel, node cards, resources, filter dropdowns |
| `components/inspector/inspector.css` | Inspector panel |
| `components/requests/requests.css` | Request and suspect cards |
| `components/timeline/timeline.css` | Timeline panel |
| `pages/StorybookPage.css` | Storybook/lab page |
