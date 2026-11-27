# higanbana-gpui

A retained-lite, GPU-rendered UI library for Windows desktop apps, written in Rust on top of a
small C11 core. You describe the UI as a tree of elements every time state changes; the library
lays it out with flexbox, paints it with one of five GPU backends, and sleeps when nothing changes.

```text
your app (View::render -> Element tree)
        |
higanbana-gpui        Rust API: elements, styles, layout (taffy), input, animation,
        |             theme, components, user shaders (WGSL -> naga)
higanbana-gpui-sys    FFI bindings (bindgen) + the WGPU backend written in Rust
        |
core/ (C11)           window (Win32), events, text (DirectWrite), atlas, draw list,
        |             images, shaders, backend switching
        v
D3D11 | D3D12 | OpenGL | Vulkan | WGPU
```

- **Platform:** Windows only (Win32 windows, DirectWrite text, HWND handles).
- **Backend selection:** `BackendKind::Auto` (default) tries D3D11, then OpenGL, then WGPU.
  Asking for a specific backend (`D3D11`, `D3D12`, `Vulkan`, `OpenGL`, `Wgpu`) tries it first and
  then falls back to the same list. `RenderCx::backends()` lists what this machine supports.
- **Switching at runtime:** `EventCx::switch_backend(kind)` moves every window to another backend
  without closing it. Images and shaders are re-created on demand.
- **Idle cost:** the loop blocks on the next event, timer, animation frame or caret blink. A frame
  is rebuilt, re-laid out or only repainted, depending on what changed.

Contents:
[Quick start](#quick-start) ·
[App and loop](#app-and-the-event-loop) ·
[Contexts](#contexts-rendercx-and-eventcx) ·
[Elements](#elements) ·
[Styling](#styling-the-styled-trait) ·
[Layout](#layout) ·
[Events, focus, keyboard](#events-focus-and-keyboard) ·
[Text and fonts](#text-and-fonts) ·
[Images and icons](#images-and-icons) ·
[Animation](#animation) ·
[Theme](#theme) ·
[Components](#components) ·
[Video](#video-feature-video) ·
[Shaders](#user-shaders-feature-shaders) ·
[Low-level API](#low-level-gfx-api) ·
[Examples](#examples) ·
[Features and tests](#cargo-features-and-tests)

## Quick start

```toml
[dependencies]
higanbana-gpui = { path = "crates/higanbana-gpui" }
```

```rust
use higanbana_gpui::prelude::*;

struct Counter {
    count: u32,
}

impl View for Counter {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        let t = cx.theme();
        div()
            .size_full()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(12.0)
            .bg(t.background)
            .text_color(t.foreground)
            .child(text(format!("Clicked {} times", self.count)).text_size(t.typography.xl))
            .child(
                div()
                    .id("inc")
                    .px(16.0)
                    .py(8.0)
                    .rounded(t.radius)
                    .bg(t.primary)
                    .text_color(t.primary_foreground)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(|v: &mut Counter, _, cx| {
                        v.count += 1;
                        cx.notify();
                    })
                    .child("Increment"),
            )
    }
}

fn main() -> higanbana_gpui::Result<()> {
    App::new().run(WindowOptions { title: "Counter".into(), ..Default::default() }, Counter { count: 0 })
}
```

The rules behind that snippet:

- Your state is a `View`. `View::render` builds an `Element<Self>` tree.
- Handlers receive `&mut YourView` plus an `EventCx`. Call `cx.notify()` when state changed and the
  tree must be rebuilt.
- Anything that must survive rebuilds (scroll offset, hover, focus, animation) is keyed by an
  `ElementId`. `.id("name")`, `("item", i)`, `String` and `u64` all convert into one.

`higanbana_gpui::prelude::*` brings in the common names: `div`, `text`, `img`, `icon`, `polygon`,
`overlay`, `scroll`, `scroll_x`, `text_field`, `on_visible`, `px`, `pct`, `auto`, `rgb`, `rgba`,
`App`, `View`, `Element`, `RenderCx`, `EventCx`, `Styled`, `Theme`, `Color`, `Transition`,
`Easing`, `Key`, `KeyEvent`, `MouseEvent`, `PointerEvent`, `ImageData`, `ImageSource`, `IconSet`,
`TextInputState`, `Anchor`, `AnchorAlign`, `Side`, `BackendKind`, `CursorStyle`, `WindowOptions`,
`Decorations`, `HitKind`, `WindowCommand`, `WindowState`, `NavDirection`, `Shader` and the
`params!` macro. Components live in `higanbana_gpui::components::*`.

## App and the event loop

`App` is a builder. `run` blocks until the window closes or a handler calls `cx.quit()`.

| Method | What it does |
| --- | --- |
| `App::new()` / `App::default()` | Auto backend, dark theme, vsync on. |
| `.backend(BackendKind)` | Preferred backend (falls back if unavailable). |
| `.theme(Theme)` | Initial theme. |
| `.fonts_dir(path)` | Folder of `.ttf`/`.otf` files loaded without installing them. Can be called more than once. |
| `.icons(IconSet)` | Adds the app's SVG icons to the built-in Lucide set. |
| `.mailbox::<V>()` | Returns a `Mailbox<V>` other threads use to send work to the view. Call before `run`. |
| `.on_window(f)` | Called once with the native window handle (HWND as `usize`). |
| `.show_stats(bool)` | Draws backend, frame time and memory in a corner. |
| `.vsync(bool)` | Presents with or without vsync. |
| `.run(WindowOptions, view)` | Opens the window and runs the loop. |

`WindowOptions { title, size, resizable, decorations, min_size }` defaults to a 960x640 resizable
window with `Decorations::Native`. `Decorations::Custom` removes the system title bar. You then mark
drag areas with `.window_drag()` and buttons with `.window_control(HitKind::Maximize)` (Windows 11
snap layouts keep working), or use the [`TitleBar`](#window) component.

Each frame does only as much work as needed: **rebuild** (call `View::render`), **relayout** or
**repaint**. A shader marked `animated(true)` asks only for a repaint, never a rebuild.

## Contexts: `RenderCx` and `EventCx`

### `RenderCx` (inside `View::render`)

| Group | Methods |
| --- | --- |
| Environment | `theme()`, `backend()`, `backends()`, `stats()`, `viewport()`, `scale()`, `window_state()`, `now()`, `waker()` |
| Focus and hover | `is_focused(id)`, `focus()` (returns `FocusInfo`), `is_hovered(id)` (the element needs `on_hover`) |
| Last layout | `bounds(id)` (needs `.track_bounds()`), `scroll_offset(id)`, `scroll_max(id)`, `scroll_view(id)` |
| Scrolling | `scroll_to(id, offset)` (applied after this frame's layout) |
| Animation | `animate`, `animate_from`, `animate_value`, `animate_value_from`, `presence`, `animate_loop` (see [Animation](#animation)) |
| Scheduling | `request_redraw_in(Duration)`, `request_animation_frame()` |
| Background work | `spawn(work, done)` |

`FocusInfo.visible` is true when focus came from the keyboard. Show focus rings only then.

### `EventCx` (handlers, timers, tasks, mailbox)

| Group | Methods |
| --- | --- |
| State | `notify()`, `is_notified()` |
| Propagation | `stop_propagation()`, `propagation_stopped()` |
| Focus | `focus(id)`, `blur()`, `focus_next()`, `focus_prev()` |
| Scrolling | `scroll_to(id, offset)`, `scroll_into_view(container, child)` |
| Timers | `spawn_timer(delay, f) -> TimerId`, `cancel_timer(id)` |
| Tasks | `spawn(work, done) -> TaskId`, `cancel_task(id)` |
| App | `switch_backend(kind)`, `set_theme(theme)`, `window_command(WindowCommand)`, `quit()` |
| Frames | `request_animation_frame()` |

```rust
// Timer: runs once, on the UI thread, with the view.
cx.spawn_timer(Duration::from_millis(300), |v: &mut MyView, cx| {
    v.debounced = true;
    cx.notify();
});

// Task: `work` runs on a thread pool, `done` runs on the UI thread with the result.
// There is no async runtime; `work` may call `runtime.block_on(fut)`.
cx.spawn(
    || std::fs::read_to_string("notes.txt").unwrap_or_default(),
    |v: &mut MyView, text, cx| {
        v.notes = text;
        cx.notify();
    },
);
```

If the window closes before a task finishes, its result is dropped.

**Mailbox.** Use it for long-lived producers such as sockets or watchers:

```rust
let mut app = App::new();
let mailbox = app.mailbox::<MyView>();
std::thread::spawn(move || {
    mailbox.send(|v: &mut MyView, cx| {
        v.online = true;
        cx.notify();
    });
});
app.run(WindowOptions::default(), MyView::default())?;
```

`Mailbox::send` returns `false` once the window is closed. `RenderCx::waker()` returns a `Waker`
that wakes the loop from any thread. The video player uses it.

`WindowCommand` is one of `Minimize`, `Maximize`, `Restore`, `ToggleMaximize`, `Close`,
`EnterFullscreen`, `ExitFullscreen` or `ToggleFullscreen`. `WindowState { maximized, minimized,
fullscreen, focused }` is readable through `cx.window_state()`.

## Elements

| Constructor | Element |
| --- | --- |
| `div()` | Flex box (row by default) with children. |
| `text(s)` | Text. Font, size and color are inherited. `&'static str` and `String` also convert into `Element` directly (`.child("Hi")`). |
| `img(ImageSource)` | Image. Without an explicit size it uses its natural size. |
| `icon("name")` | Registered SVG icon, 16 px, in the inherited text color. |
| `polygon(points, color)` | Filled polygon. Points are fractions (0..1) of its own box. Rasterized on the CPU and cached. |
| `scroll(id)` | Vertical scroll column. Its offset persists by `id`. |
| `scroll_x(id)` | Horizontal scroll row (Shift + wheel or a horizontal touchpad). |
| `text_field(id, &state, lens)` | Single-line text field bound to a `TextInputState`. |
| `overlay(anchor, content)` | Content painted on a layer above the whole tree (portal). |
| `on_visible(margin, f)` | Invisible layer that calls `f` once when its parent comes within `margin` px of the visible area (lazy loading). |

Methods every `Element` has:

- `.id(id)`, `.child(e)`, `.children(iter)`
- `.when(cond, |e| ...)`, `.when_some(opt, |e, v| ...)`
- `.hover(|style| ...)`: style while the mouse is over it, with no relayout.
- `.on_click(|v, &MouseEvent, cx| ...)`
- Interaction: `.focusable()`, `.track_focus(id)`, `.focus(|style| ...)`, `.disabled(bool)`,
  `.disabled_style(|style| ...)`, `.on_key_down`, `.on_char`, `.on_mouse_down`, `.on_mouse_up`,
  `.on_mouse_move`, `.on_mouse_move_passive`, `.on_hover`, `.on_double_click` and
  `.on_navigate` (mouse back/forward buttons)
- `text_field` only: `.caret_color(c)`, `.selection_color(c)`, `.placeholder_color(c)`
- `.style()` returns the current `Style`.

### Text fields

```rust
struct Form { name: TextInputState }

// in render:
text_field("name", &self.name, |v: &mut Form| &mut v.name)
    .w(240.0)
    .px(8.0)
    .py(6.0)
    .border(1.0, theme.input)
    .rounded(theme.radius)
```

`TextInputState::new()`, `with_text(s)` and `.placeholder(s)` create one. Read it with `text()`,
`is_empty()`, `caret()`, `selection()`, `has_selection()` and `selected_text()`. Change it with
`set_text`, `set_placeholder`, `set_caret(i, extend)`, `select_all`, `select_word_at`, `insert`,
`backspace`, `delete`, `move_left`, `move_right`, `move_home`, `move_end`, `copy`, `cut` and
`paste`. The field handles keys (`handle_key`, `handle_char` returning `InputOutcome`), grapheme
cursor movement, the clipboard, mouse selection and a blinking caret (`CARET_BLINK_MS` = 530).
For a styled input with placeholder, password and submit, use the `Input` component.

### Overlays

```rust
overlay(Anchor::below("menu-button").align(AnchorAlign::End), menu_panel)
    .gap(4.0)
    .offset(0.0, 2.0)
    .match_width()
    .on_dismiss(|v: &mut MyView, cx| {
        v.menu_open = false;
        cx.notify();
    })
```

The anchors are `Anchor::center()`, `Anchor::window(x, y)` (fractions of the window),
`Anchor::below(id)`, `above`, `left_of`, `right_of` and `Anchor::at(point)` for context menus.
`.align(AnchorAlign::Start | Center | End)` aligns along the anchor side. If the overlay doesn't fit,
it flips to the opposite `Side`. Overlay modifiers: `.modal()`, `.backdrop(color)`, `.gap`,
`.offset`, `.match_width()` and `.on_dismiss`. Escape or a click outside calls `on_dismiss`.
`Overlay<V>` converts into `Element<V>`.

### Scrolling and virtual lists

`scroll("list")` keeps its offset across rebuilds. Read it with `cx.scroll_offset("list")` and
change it with `cx.scroll_to`. `.rerender_on_scroll()` rebuilds the view while it scrolls. For long
lists, use [`VirtualList`](#list), which builds only the visible range plus one screen on each side.

## Styling: the `Styled` trait

`Element`, `Style` (inside `.hover`/`.focus` closures) and a few components implement `Styled`.
Every method takes and returns `self`. Lengths accept `f32` (pixels), `px(v)`, `pct(v)` (0..100) or
`auto()`.

| Group | Methods |
| --- | --- |
| Display and direction | `flex`, `hidden`, `flex_row`, `flex_col`, `flex_row_reverse`, `flex_col_reverse`, `flex_wrap` |
| Flex item | `flex_grow`, `flex_shrink_0`, `flex_1`, `flex_none`, `basis(len)` |
| Gap and padding (px) | `gap`, `gap_x`, `gap_y`, `p`, `px`, `py`, `pt`, `pr`, `pb`, `pl` |
| Margin (length) | `m`, `mx`, `my`, `mt`, `mr`, `mb`, `ml` (negative values allowed) |
| Size | `w`, `h`, `size`, `w_full`, `h_full`, `size_full`, `min_w`, `min_h`, `max_w`, `max_h` |
| Cross-axis alignment | `items_start`, `items_center`, `items_end`, `items_stretch`, `items_baseline`, `self_start`, `self_center`, `self_end` |
| Main-axis justification | `justify_start`, `justify_center`, `justify_end`, `justify_between`, `justify_around`, `justify_evenly` |
| Position | `relative`, `absolute`, `top`, `right`, `bottom`, `left`, `inset`, `overflow_hidden` |
| Fill | `bg(color)`, `bg_gradient(LinearGradient)`, `shader(&Shader, params)` |
| Border and corners | `border(width, color)`, `border_color`, `border_dashed`, `rounded(r)`, `rounded_corners(Corners)`, `rounded_full` |
| Effects | `shadow(color, blur)`, `opacity(0..1)` |
| Transform (paint and hit test only) | `translate(x, y)`, `translate_x`, `translate_y`, `scale(k)`, `rotate(turns)` (icons, in 7.5° steps) |
| Images | `object_fit(ObjectFit::Fill \| Contain \| Cover)`, `zoom(k)` |
| Cursor | `cursor(CursorStyle)`, `cursor_pointer` |
| Text (inherited) | `text_color`, `text_size`, `font_weight(u16)`, `font_bold`, `italic`, `font_family(&str)`, `whitespace_nowrap`, `line_clamp(lines)` |
| Window | `window_drag()`, `window_control(HitKind)` |
| Tracking | `track_bounds()` (enables `cx.bounds(id)`), `rerender_on_scroll()` |

`scale` only applies in the base style. To animate it, use `cx.animate`. `CursorStyle` is one of
`Arrow`, `Pointer`, `Text`, `Crosshair`, `NotAllowed`, `ResizeHorizontal`, `ResizeVertical`,
`Move`, `Wait` or `None`.

Gradients live in `higanbana_gpui::style`:

```rust
use higanbana_gpui::style::{GradientDir, LinearGradient};

div().bg_gradient(LinearGradient::scrim(GradientDir::Up, rgba(0x000000cc), Color::TRANSPARENT))
// or LinearGradient::new(GradientDir::Right, vec![(0.0, a), (0.5, b), (1.0, c)])
```

Colors: `rgb(0xRRGGBB)`, `rgba(0xRRGGBBAA)`, `Color::from_hex("#rgb" | "#rrggbb" | "#rrggbbaa")`,
`Color::WHITE`, `BLACK` and `TRANSPARENT`, plus `.alpha(a)` and `.fade(k)`. Colors use straight
(non-premultiplied) alpha.

## Layout

Layout is flexbox, done by [taffy](https://crates.io/crates/taffy) (flexbox only). The defaults match
CSS: `div()` is a row with `flex-shrink: 1`. `absolute()` children are placed against the nearest
positioned ancestor using `top`, `right`, `bottom`, `left` and `inset`. Text is measured through
DirectWrite and cached. `.hover(..)` styles and `translate` don't change layout, so they only
repaint.

## Events, focus and keyboard

- **Mouse:** `MouseEvent { position, button, mods }` for clicks. `PointerEvent { position, local,
  bounds, button, mods, pressed, text_index }` for down, up and move. `fraction_x()` gives a 0..1
  position inside `bounds`, which is useful for sliders. A mouse-down on an element captures the
  pointer until mouse-up, so drags keep working outside it.
- **Keyboard:** `on_key_down(|v, &KeyEvent, cx| ...)`. The event bubbles from the focused element
  to the root until something calls `cx.stop_propagation()`. `KeyEvent { key, mods, repeat }`.
  `Key` is one of `Tab`, `Enter`, `Escape`, `Backspace`, `Delete`, `Space`, `Left`, `Right`, `Up`,
  `Down`, `Home`, `End`, `PageUp`, `PageDown`, `F(n)`, `Char(c)` or `Other(vk)`. `ev.ctrl('c')`
  tests for a Ctrl shortcut. Typed text arrives through `on_char`.
- **Focus:** `.focusable()` (needs an `id`) or `.track_focus(id)`. Tab and Shift+Tab move focus.
  Enter or Space on a focused element with `on_click` fires the click. `.focus(|s| ...)` styles the
  focused state.
- **Disabled:** `.disabled(true)` removes click, focus and hover and shows the not-allowed cursor.
- **Hover:** `.hover(|s| ...)` restyles without a rebuild. `on_hover(|v, entered, cx| ...)` makes
  `cx.is_hovered(id)` work.
- **Back/forward:** `.on_navigate(|v, NavDirection::Back | Forward, cx| ...)`.

## Text and fonts

- The default family comes from `theme.typography.family` (`"Segoe UI"`). Sizes come from
  `typography.xs`, `sm`, `base`, `lg`, `xl` and `xxl` (12, 14, 16, 18, 20, 24). The root text style
  uses `typography.sm`, `foreground` and weight 400.
- `App::new().fonts_dir("assets/fonts")` registers every `.ttf`/`.otf` in the folder with the
  context before the window opens. A missing folder is ignored. If the theme family still doesn't
  exist afterwards, the library falls back to Segoe UI.
- Use `font_family("Name")`, `font_weight(100..=900)`, `italic()`, `whitespace_nowrap()` and
  `line_clamp(n)` (ends with "…") on any ancestor. Text wraps at the available width by default.
- Lower level: `font_files_in(dir)`, `Context::add_font_file`, `Context::has_font_family`,
  `Context::font(&FontSpec)` and `Context::measure_text`.

## Images and icons

```rust
let cover: ImageSource = ImageData::from_rgba8(w, h, rgba_pixels); // straight-alpha RGBA8
img(cover.clone()).w(96.0).h(128.0).rounded(8.0).object_fit(ObjectFit::Cover)
```

- `ImageSource` is an `Rc<ImageData>`, so it is cheap to clone. Pixels leave Rust memory as soon as
  the core uploads them. After a backend switch a static image comes back empty, and the view has
  to create a new `ImageData`.
- `ImageData::dynamic(w, h)` plus `set_frame(w, h, pixels)` is for changing frames (video), with
  `set_recycler` to reuse buffers. Other accessors are `size()`, `id()` and `is_dynamic()`.
- `premultiply_rgba8(&mut pixels)` converts to the core's format when you go below `img`.

Icons are SVGs rasterized at physical pixel size and cached:

```rust
icon("play").size(20.0).text_color(theme.primary)

App::new().icons(IconSet::new().add("logo", include_bytes!("../assets/logo.svg")))
// or the macro: higanbana_gpui::icon_set!["logo" => "../assets/logo.svg"]
```

The built-in set covers the Lucide icons the apps use, in kebab-case: `"arrow-left"`,
`"chevron-down"`, `"circle-check"`, `"volume-2"` and so on (see `icons/lucide`). Related functions
are `IconSet::lucide()`, `icon::exists(name)` and `icon::register(set)`.

## Animation

Animations are tweens keyed by id and queried during render. While something animates, the app
rebuilds every frame. When it settles, the app goes back to sleep.

```rust
let open = cx.animate("panel", if self.open { 1.0 } else { 0.0 }, Transition::ms(200));
let color = cx.animate_value("tint", target_color, Transition::default()); // f32, Color, Point, Size, Rect
let x = cx.animate_from("intro", -40.0, 0.0, Transition::spring(170.0, 26.0));

// Presence: Some(0..1) while visible or animating out, None once fully gone.
if let Some(p) = cx.presence("toast", self.show, Transition::ms(150)) {
    root = root.child(card.opacity(p).translate_y((1.0 - p) * 8.0));
}

// Endless 0..1 progress (spinners, pulses). Stops when render stops asking.
let t = cx.animate_loop("pulse", Transition::ms(1000).easing(Easing::EaseInOut).yoyo());
```

- `Transition::new(duration, easing)`, `Transition::ms(ms)` (ease-out) and `Transition::default()`
  (150 ms, ease-out).
- Springs: `Transition::spring(stiffness, damping)` or
  `Transition::with_spring(anim::Spring::new(..))` (`Spring::DEFAULT` is 170 / 26).
  Changing the target keeps the velocity.
- Modifiers: `.easing(Easing)`, `.delay(ms)`, `.repeat(Repeat::Once | Times(n) | Forever)`, `.yoyo()`
  and `.fps(n)` to cap frame rate for slow motions and save CPU.
- `Easing::Linear`, `EaseIn`, `EaseOut`, `EaseInOut` or `Custom(fn(f32) -> f32)`.
- Implement `anim::Animatable` to animate your own types.
- `cx.request_redraw_in(d)` schedules a rebuild (clocks, carousels) without a timer.

## Theme

`Theme` holds shadcn/ui-style tokens: `background`, `foreground`, `card`, `popover`, `primary`,
`secondary`, `muted`, `accent` and `destructive` (each with `_foreground`), `border`, `input`,
`ring`, `primary_hover`, `primary_active`, `secondary_hover`, `group_box`, `list_hover`,
`list_active`, `list_active_border`, `success`, `warning` and `info` (with `_foreground`), `link`,
`selection`, `overlay`, the `sidebar*` tokens, `title_bar`, `title_bar_border`, `skeleton`,
`switch` and `scrollbar_thumb`. It also has `base: BaseColors` (red, green, blue, yellow, magenta
and cyan, plus `_light` variants), `radius`, `radius_lg`, `spacing` (`spacing.s(n)`) and
`typography`.

```rust
App::new().theme(Theme::light());
cx.set_theme(current.toggled());                      // in a handler: dark <-> light
let t = Theme::from_json(include_str!("theme.json"))?; // gpui-kit theme JSON format
```

`Theme::from_json` reads the first theme in the file, or the one marked `is_default`. Missing keys
or invalid colors keep the default value for that mode.

## Components

Components are shadcn/ui and gpui-kit style builders in `higanbana_gpui::components`. Each one is
plain data until you call `.render(cx)` from the `Component<V>` trait. That call reads theme tokens
and focus, and may animate. Your view owns the state. Components hand changes back through
callbacks or a **lens** (`|v: &mut MyView| &mut v.field`). Interactive components need an id.

```rust
use higanbana_gpui::components::*;

Button::new("save", "Save")
    .variant(Variant::Default)
    .icon(icon("check"))
    .on_click(|v: &mut MyView, _, cx| { v.saved = true; cx.notify(); })
    .render(cx)
```

The shared types are `Variant`, which is one of `Default`, `Secondary`, `Outline`, `Ghost`,
`Destructive` or `Link`, and `ComponentSize` (`Xs`, `Sm`, `Md`, `Lg`, `Icon`, `Regular`, `Compact`,
with `.height()`). The helpers are `focus_ring(style, color)` and `variant_colors(theme, variant)`.
Many components have a `.kit()` method that switches to the gpui-kit look.

### `basic`

| Component | Constructor and main methods | Example |
| --- | --- | --- |
| `Button<V>` | `new(id, label)`, `.variant`, `.size`, `.icon(el)`, `.trailing_icon(el)`, `.disabled`, `.full_width`, `.on_click` | `Button::new("ok", "OK").variant(Variant::Outline).render(cx)` |
| `Badge` | `new(label)`, `.variant` | `Badge::new("New").variant(Variant::Secondary).render(cx)` |
| `Tag` | `Tag::secondary(label)` / `Tag::outline(label)`, `.small`, `.xsmall` | `Tag::outline("1080p").small().render(cx)` |
| `Card<V>`, `CardHeader` | `Card::new().header(CardHeader::new(t).description(d)).content(el).footer(el)` | `Card::new().header(CardHeader::new("Plan")).content(body).render(cx)` |
| `Separator` | `Separator::horizontal()` / `vertical()` (`Orientation`) | `Separator::horizontal().render(cx)` |
| `Skeleton` | `new(id)`, `.w`, `.h`, `.rounded_full` (pulses at `PULSE_MS`) | `Skeleton::new("sk").w(200.0).h(16.0).render(cx)` |
| `Avatar` | `new(name)`, `.image(src)`, `.size(ComponentSize)`. Shows `initials(name)` without an image. | `Avatar::new("Ada Lovelace").render(cx)` |
| `Progress<V>` | `new(id)`, `.value(0..100)`, `.size(ProgressSize::Small \| Medium \| Large)`, `.color` | `Progress::new("dl").value(42.0).render(cx)` |
| `Spinner<V>` | `new(id)`, `.size(px)`, `.color`, `.icon(name)` | `Spinner::new("busy").size(20.0).render(cx)` |
| `Kbd` | `new("ctrl-k")`, `.outline` (`Kbd::label` turns it into `Ctrl+K`) | `Kbd::new("ctrl-k").render(cx)` |
| `Link<V>` | `new(id, label)`, `.on_click(\|v, cx\| ..)` | `Link::new("docs", "Docs").on_click(open).render(cx)` |

### `form`

| Component | Constructor and main methods | Example |
| --- | --- | --- |
| `Input<'a, V, L>` | `new(id, &state, lens)`, `.placeholder`, `.width`, `.height`, `.prefix(icon)`, `.password`, `.invalid`, `.disabled`, `.kit`, `.on_change(\|v, &str, cx\|)`, `.on_submit` | `Input::new("email", &self.email, \|v: &mut F\| &mut v.email).placeholder("you@x.com").render(cx)` |
| `Checkbox<V>` | `new(id, state)` (`bool` or `CheckState::Unchecked \| Checked \| Indeterminate`), `.label`, `.disabled`, `.on_change(\|v, bool, cx\|)` | `Checkbox::new("terms", self.terms).label("Accept").render(cx)` |
| `Switch<V>` | `new(id, checked)`, `.kit`, `.disabled`, `.on_change(\|v, bool, cx\|)` | `Switch::new("wifi", self.wifi).on_change(\|v, on, cx\| { v.wifi = on; cx.notify() }).render(cx)` |
| `Slider<V>` | `new(id, value)`, `.range(min, max)`, `.step`, `.width`, `.kit`, `.disabled`, `.on_change`, `.on_release` | `Slider::new("vol", self.vol).range(0.0, 100.0).step(1.0).render(cx)` |
| `RadioGroup<V>` | `new(id, Option<usize>)`, `.item`, `.items(iter)`, `.horizontal`, `.kit`, `.on_change(\|v, usize, cx\|)` | `RadioGroup::new("q", Some(0)).items(["720p", "1080p"]).render(cx)` |

### `overlay`

| Component | Constructor and main methods | Example |
| --- | --- | --- |
| `Dialog<V>` | `new(id, open)`, `.title`, `.description`, `.child`, `.footer`, `.width`, `.kit`, `.on_open_change(\|v, bool, cx\|)`. `Dialog::open(cx, id)` moves focus inside. | `Dialog::new("del", self.open).title("Delete?").on_open_change(\|v, o, cx\| { v.open = o; cx.notify() }).render(cx)` |
| `Tabs<V>` | `new(id, active, on_change)`, `.tab(label, content)`. Arrow keys, Home and End move between tabs. | `Tabs::new("t", self.tab, \|v, i, cx\| { v.tab = i; cx.notify() }).tab("A", a).tab("B", b).render(cx)` |
| `TabBar<V>` | `new(id, active, on_change)`, `.tab(label)`, `.underline` / `.pill` (triggers only) | `TabBar::new("tb", self.tab, set_tab).pill().tab("All").tab("Fav").render(cx)` |
| `Select<V>`, `SelectState` | `new(id, &state, lens)`, `.item`, `.items`, `.placeholder`, `.width`, `.max_height`, `.on_change(\|v, usize, cx\|)` | `Select::new("res", &self.res, \|v: &mut F\| &mut v.res).items(["720p", "1080p"]).render(cx)` |
| `Popover<V>`, `PopoverState`, `PopoverAnchor` | `new(id, &state, lens, trigger)`, `.anchor(TopLeft \| TopRight \| BottomLeft \| BottomRight)`, `.w`, `.padding`, `.content(\|cx\| el)` | `Popover::new("p", &self.pop, \|v: &mut F\| &mut v.pop, btn).content(\|_\| text("Hi")).render(cx)` |
| `DropdownMenu<V>` | `new(id, &PopoverState, lens, trigger)`, `.anchor`, `.item(label, checked, pick)`, `.max_height` | `DropdownMenu::new("m", &self.pop, \|v: &mut F\| &mut v.pop, btn).item("Copy", false, copy).render(cx)` |
| `Tooltip<V>`, `TooltipState` | `new(id, &state, lens, trigger, label)`, `.side(Side)`, `.delay(Duration)`, `.kit` | `Tooltip::new("tip", &self.tip, \|v: &mut F\| &mut v.tip, btn, "Save").render(cx)` |
| `Command<'a, V>`, `CommandState`, `CommandGroup`, `CommandItem` | `new(id, &state, lens)`, `.placeholder`, `.max_h`, `.group(CommandGroup::new(label).item(CommandItem::new(id).child(el)))`, `.empty(el)`, `.on_query(\|v, &str, cx\|)`, `.on_confirm(\|v, (group, item), cx\|)`. The app does the filtering. | `Command::new("cmd", &self.cmd, \|v: &mut F\| &mut v.cmd).on_query(search).render(cx)` |
| `Toasts`, `Toast`, `Toaster<'a, V>` | `Toasts::new().max_visible(n)`, `.push(Toast::new(t).description(d).variant(v).duration(d))`, `.push_success(msg)`, `.push_error(msg)`, `.dismiss(id)`. Render `Toaster::new(id, &toasts, lens)` once at the root (bottom right). | `self.toasts.push_success("Saved")` |
| `Notifier<'a, V>` | `new(id, &toasts, lens)`: the same `Toasts` queue in the kit's top-right notification style. `Toast::tone(ToastTone::Info \| Success \| Warning \| Error)` picks the icon and color (`NOTIFICATION_DURATION` = 5 s). | `Notifier::new("n", &self.toasts, \|v: &mut F\| &mut v.toasts).render(cx)` |

### `nav`

| Component | Constructor and main methods | Example |
| --- | --- | --- |
| `Sidebar<'a, V>`, `SidebarState`, `SidebarGroup`, `SidebarItem` | `Sidebar::new(id, &state, lens).header(el).group(SidebarGroup::new(label).item(SidebarItem::new(id, label).icon(name).active(bool).on_click(..))).footer(el)`. It collapses to icons (`state.collapsed`, `SIDEBAR_WIDTH` 255 / `SIDEBAR_COLLAPSED_WIDTH` 48) and animates the width. | `Sidebar::new("nav", &self.side, \|v: &mut F\| &mut v.side).group(g).render(cx)` |
| `SidebarToggle<V>` | `new(id, collapsed)`, `.on_click` | `SidebarToggle::new("tog", self.side.collapsed).on_click(toggle).render(cx)` |
| `Breadcrumb<V>` | `new(id)`, `.link(label, go)`, `.item(label)` (last, not clickable) | `Breadcrumb::new("bc").link("Home", home).item("Settings").render(cx)` |

### `list`

| Component | Constructor and main methods | Example |
| --- | --- | --- |
| `VirtualList` | `new(id, heights: Arc<[f32]>)`, `.gap`, `.render(cx, \|index, cx\| element)`. Helpers: `visible_range`, `item_offset` and `Range`. | `VirtualList::new("rows", self.heights.clone()).gap(4.0).render(cx, \|i, _\| row(i))` |

### `carousel`

| Component | Constructor and main methods | Example |
| --- | --- | --- |
| `Carousel<V>`, `CarouselState` | `Carousel::new(id, &state, lens).items(vec).item_width(w).gap(g).controls(true).on_change(..)`, rendered with the inherent `.render(cx)`. `CarouselState::new(len).looping(bool).auto_advance(Duration)` and `next`, `prev`, `go`, `current(now)`, `set_paused`. Auto-advance is derived from the clock, with no timer. | `Carousel::new("hero", &self.car, \|v: &mut F\| &mut v.car).items(slides).controls(true).render(cx)` |

### `window`

| Component | Constructor and main methods | Example |
| --- | --- | --- |
| `TitleBar<V>` | `new(title)`, `.leading(el)`, `.content(el)`, `.trailing(el)`, `.kit`. For `Decorations::Custom`: it drags the window, double-click maximizes, has min/max/close buttons and is hidden in fullscreen. Constants: `TITLE_BAR_HEIGHT` 40, `KIT_TITLE_BAR_HEIGHT` 34, `CAPTION_BUTTON_WIDTH`. | `TitleBar::new("My app").kit().render(cx)` |

### `video` (feature `video`)

| Component | Constructor and main methods | Example |
| --- | --- | --- |
| `VideoPlayer<'s, V>`, `VideoState` | `VideoPlayer::new(id, &mut state, lens).fit(ObjectFit)`. Controls hide themselves after `HIDE_AFTER` (2.5 s). It has keyboard shortcuts, double-click fullscreen, and speed, audio and subtitle menus. | `VideoPlayer::new("player", state, \|v: &mut F\| v.video.as_mut().unwrap()).render(cx)` |
| `Seekbar<'a, V>`, `SeekbarState` | `new(id, &state, lens)`, `.duration`, `.position`, `.buffered`, `.marks(vec)`, `.played_color`, `.hover_content(\|secs, x, w\| el)`, `.on_seek`, `.on_drag` | `Seekbar::new("sb", &self.bar, \|v: &mut F\| &mut v.bar).duration(d).position(p).render(cx)` |
| `VideoSurface<'s, V>`, `ClickGate` | `new(id, &mut player, gate_lens)`, `.fit`, `.cursor_hidden`, `.on_click`, `.on_double_click` (`DOUBLE_CLICK` window) | `VideoSurface::new("surf", &mut player, \|v: &mut F\| &mut v.gate).render(cx)` |
| `AutoHide` | `new(after, now)`, `poke`, `tick`, `hidden(hold)`, `idle` | building block for custom players |

## Video (feature `video`)

`higanbana_gpui::video` plays media with **libmpv**, loaded at runtime (`libmpv-2.dll`; it is not
linked). On D3D11, mpv draws with OpenGL straight into a D3D11 texture that the UI shows
(`RenderPath::Gpu`). On other backends, or when interop fails, it renders on the CPU and the frame
becomes a dynamic image (`RenderPath::Cpu`).

```rust
use higanbana_gpui::video::Player;

let player = Player::new(cx.waker())?; // create it in render: it needs the window's waker
player.load("movie.mkv");
self.video = Some(VideoState::new(player));
```

- `Player`: `new(waker)`, `with_options(waker, PlayerOptions)`, `headless(options)` (no frames, for
  thumbnails), `detached()` (for UI tests). Commands are `load`, `load_at`, `play`, `pause`,
  `toggle_pause`, `seek`, `seek_preview`, `seek_relative`, `set_volume`, `toggle_mute`,
  `set_speed`, `set_audio_track`, `set_sub_track`, `set_property` and `command_blocking`. State
  comes from `state() -> PlayerState` (`status: Status`, `position`, `duration`, `volume`, `muted`,
  `speed`, `audio_tracks`/`sub_tracks: Vec<Track>`, `chapters: Vec<Chapter>`, `cache_ahead`,
  `buffering` and more). Frames come from `frame()` and `render_path()`.
- `PlayerOptions::default().set("cache", "yes").gpu(false)`. `PlayerOptions::empty()` and
  `DEFAULT_OPTIONS` are also available. `HIGANBANA_VIDEO_GPU=0` turns the GPU path off by default.

## User shaders (feature `shaders`)

Any element can be filled by a WGSL fragment function. The library translates it at runtime with
[naga](https://crates.io/crates/naga) for every backend: HLSL compiled with `D3DCompile` for
D3D11 and D3D12, GLSL for OpenGL, SPIR-V for Vulkan, and WGSL as-is for WGPU.

### The WGSL contract

You write only one function:

```wgsl
fn shade(p: ShadeInput) -> vec4<f32> {
    // return a NON-premultiplied color; the library clips rounded corners (SDF anti-aliasing),
    // multiplies by inherited opacity and blends premultiplied, like regular quads.
}
```

The library builds the module as **prelude + your code + entry points**. The prelude provides:

```wgsl
struct ShadeInput {
    uv: vec2<f32>,     // 0..1 inside the rectangle
    px: vec2<f32>,     // pixel position inside the rectangle
    size: vec2<f32>,   // rectangle size in pixels
    screen: vec2<f32>, // pixel position in the window (origin top-left on every backend)
    time: f32,         // seconds since the Shader was created
    scale: f32,        // DPI scale (physical pixels per logical pixel)
    radius: vec4<f32>, // corner radii tl, tr, br, bl in pixels
}

fn param(i: u32) -> vec4<f32>                                     // app parameter 0..15 (higher reads 15)
fn backdrop(screen_px: vec2<f32>) -> vec4<f32>                    // what is already drawn behind (bilinear)
fn backdrop_blur(screen_px: vec2<f32>, radius: f32) -> vec4<f32>  // 5x5 binomial blur of the backdrop
fn rounded_sdf(px: vec2<f32>, size: vec2<f32>, radius: vec4<f32>) -> f32 // signed distance, < 0 inside
```

Rules and limits:

- `px`, `size` and `radius` are in **physical** pixels. Multiply logical values passed in params by
  `p.scale`.
- `backdrop` and `backdrop_blur` return transparent unless the shader was created with
  `.backdrop(true)`.
- `backdrop_blur` clamps its radius to 64 px, the margin the backends copy around the rectangle.
  **Known limitation:** its 25 taps are r/2 apart and read the full-resolution texture with no
  mipmaps. Above about 20 px of radius, fine detail such as text and thin lines shows up as
  "ghost" copies. Keep the radius at or below 20 px for now. Averaging a second grid offset by half
  a step, as `liquid_glass.wgsl` does, smooths it further.
- You may not declare bindings, resources or entry points. Use `param(i)` and `backdrop()`. The
  `hg_` prefix is reserved.

### Rust API

```rust
use higanbana_gpui::prelude::*; // Shader, params!

// Translate once and keep it (in the view, a thread_local, ...). Clones share the translation.
let glass = Shader::wgsl(include_str!("glass.wgsl"))?
    .backdrop(true)   // copy what is behind the rectangle before drawing it
    .animated(true);  // ask for continuous frames while it is visible

div()
    .size(300.0)
    .rounded(24.0)
    .shader(&glass, params![[32.0, 8.0, 24.0, 1.0], Color::WHITE.alpha(0.08)])
    .child(text("Hello"))
```

- `Shader::wgsl(src) -> Result<Shader, ShaderError>` composes and translates once. `Shader` is an
  `Rc` handle and cheap to clone. The GPU cache is keyed by the handle, so don't call `wgsl` every
  frame.
- Builders: `.backdrop(bool)` and `.animated(bool)`. Accessors: `is_backdrop()`, `is_animated()`,
  `time()`, `source()`, `translations()`, `same(&other)` and `last_error()`.
- `params![a, b, ...]` builds `ShaderParams` (`[[f32; 4]; 16]`). Each value goes through
  `Into<[f32; 4]>`, so `[f32; 4]` and `Color` both work. Unused slots are zero, and more than 16
  values panics.
- `Styled::shader(&Shader, ShaderParams)` replaces `bg` and `bg_gradient` for that element.
  **Paint order:** shadow, then shader, then border, then children. A shadow on the shader element
  itself is drawn first, so it lands inside its own backdrop. A glass panel usually has no
  `shadow`.
- **Animation:** an `animated(true)` shader asks only for a repaint, never a rebuild, and only when
  it was actually painted this frame and the window is not minimized. Unfocused or covered windows
  keep animating, as video does. Off-screen shaders don't.
- **Cost:** translation runs once per `Shader`. Native compilation runs once per (shader, backend)
  on first draw. Apps without shaders pay nothing. Each backdrop shader copies only its rectangle
  plus the blur margin.

### Errors

`ShaderError` has four variants:

| Variant | When |
| --- | --- |
| `Parse { line, column, message }` | WGSL syntax or type error. |
| `Validate { line, column, message }` | naga validation or a contract rule, such as declaring a binding. |
| `Native(String)` | Backend compiler (D3DCompile, GL driver, Vulkan pipeline, WGPU). It appears on first draw. |
| `Unsupported` | Built without the `shaders` feature. |

`line` and `column` start at 1 and count only **your** code. `line == 0` means the error is in
library code, for example a missing `fn shade` or a wrong signature. `Parse` and `Validate` come
back from `Shader::wgsl`. A `Native` error is found later. In that case the element paints as a
**magenta** rectangle with the same corners, the error is printed once (`eprintln!`), and
`shader.last_error()` returns it. Errors are tracked per backend: after
`cx.switch_backend(..)` the library clears them and compiles again on the new backend.

With `default-features = false` (no `shaders`), the API still exists, so the app needs no `cfg`.
`Shader::wgsl` returns `Err(ShaderError::Unsupported)`, and the app can fall back to a plain `bg`.

### Example: LiquidGlass

`examples/gallery/sections/liquid_glass.rs` and `liquid_glass.wgsl` build a glass panel using only
the public API. The edge refracts (the backdrop is offset along the SDF normal across a bevel),
with chromatic dispersion, blur, a tint and a rotating specular highlight:

```rust
pub fn liquid_glass<V: View>(p: GlassParams) -> Element<V> {
    let base = div().rounded(p.radius); // no shadow: it would end up in the backdrop
    match GLASS.with(|s| s.clone()) {   // thread_local Option<Shader>, translated once
        Some(s) => base.shader(&s, params![[p.refraction, p.blur, p.bevel, p.shine], p.tint]),
        None => base.bg(p.tint.alpha(0.35)).border(1.0, Color::WHITE.alpha(0.3)),
    }
}
```

`examples/elements/shader.rs` is the minimal version, an animated cosine-palette gradient behind
text.

## Low-level gfx API

Most apps never need this level. It is useful for tests, tools and custom renderers.

- `Context::new(BackendKind)`, `backend()`, `backend_info()`, `switch_backend(kind)` and `stats()`
  (`Stats { frame_ms, gpu_bytes, arena_bytes, draw_calls, atlas_entries }`). `enumerate()` returns
  `Vec<BackendInfo { kind, name, adapter }>`. `BackendKind::label()` gives a short name.
- `Window::new(&ctx, &options)` with `poll_event`, `wait_event`, `wait_event_timeout`, `present`,
  `scale`, `logical_size`, `physical_size`, `set_cursor`, `command`, `state`, `waker` and
  `native_handle`. `Event` has the variants `Close`, `Resize`, `MouseMove`, `MouseDown`, `MouseUp`,
  `Scroll`, `KeyDown`, `KeyUp`, `Char`, `WindowState` and `Wake`.
- `DrawList` with `quad(&Quad)`, `image`, `text`, `shader(&GpuShader, &ShaderDraw)`, `layer`,
  `clip`, `push_clip` and `pop_clip`. A shader draw closes the current layer: earlier draws stay
  behind it and later draws go on top.
- `Context::shader(&Shader) -> GpuShader`. `ShaderDraw { rect, radius, time, scale, opacity,
  params }` is in physical pixels.
- `paint::Canvas` is the painter's target trait (`quad`, `image`, `text`, `push_clip`, `pop_clip`,
  `layer`, `shader(rect, radius, &ShaderFill, opacity)`). `paint::GpuCanvas` implements it over a
  `DrawList`. `ShaderFill { shader, params }` is what `Styled::shader` stores.
- `gfx::clipboard::{get_text, set_text}`, `Image` (GPU texture) and `VideoTarget` (D3D11/GL
  interop for video).
- `Tree<V>` and `layout::LayoutEngine` are the retained tree and the layout engine, exposed for
  tests.

## Examples

```sh
# All base elements: scroll, cards, images, text field, tooltip, dialog,
# live backend and theme switching, WGSL shader card.
cargo run -p higanbana-gpui --example elements

# Component gallery (shadcn docs style), including the LiquidGlass section.
cargo run -p higanbana-gpui --example gallery

# Video player (needs libmpv-2.dll next to the exe or on PATH).
cargo run -p higanbana-gpui --features video --example video_player -- <file or URL>
```

The gallery reads environment variables for screenshots: `HGUI_GALLERY_SECTION=<index>`,
`HGUI_GALLERY_STATE=dialog|select|toast|backend|popover`, `HGUI_GALLERY_THEME=light` and
`HGUI_GALLERY_BACKEND=vulkan|opengl|d3d11|d3d12|wgpu`. In the `elements` example,
`HGUI_DEMO_OPEN=1` opens the tooltip and the dialog.

## Cargo features and tests

| Feature | Default | Effect |
| --- | --- | --- |
| `wgpu` | yes | Registers the WGPU backend (Rust, in `higanbana-gpui-sys`) with the core. |
| `shaders` | yes | User WGSL shaders, translated by `naga` at runtime. Off: `Shader::wgsl` returns `ShaderError::Unsupported`. |
| `video` | no | `video` module and `components::video` (libmpv loaded at runtime via `libloading`). |

```sh
cargo test -p higanbana-gpui                       # unit tests + GPU integration tests
cargo test -p higanbana-gpui --test shader_backends # user shaders on each available backend
cargo test -p higanbana-gpui --test shader_wgpu     # WGPU shaders, in their own process
cargo test -p higanbana-gpui --features video       # video tests (need libmpv for the GPU/player ones)
```

Integration tests in `tests/` open real windows (`gpu.rs`, `fonts.rs`, `dynamic_image.rs`,
`shader_backends.rs`, `shader_wgpu.rs`, `video*.rs`). The WGPU shader test runs in its own process
because creating a WGPU swapchain next to Vulkan fails intermittently. Unit tests sit next to the
code (`tests*.rs` in each module) and use recording canvases and measurers, so they need no GPU.
