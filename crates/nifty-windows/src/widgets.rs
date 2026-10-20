//! The common control behind each Nifty widget, and how it is sized and updated.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::path::Path;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    DRAW_TEXT_FORMAT, DT_CALCRECT, DT_EXPANDTABS, DT_NOPREFIX, DT_SINGLELINE, DrawTextW, GetDC,
    GetTextMetricsW, HDC, HFONT, InvalidateRect, ReleaseDC, TEXTMETRICW,
};
use windows::Win32::System::SystemServices::{
    SS_CENTER, SS_LEFT, SS_NOPREFIX, SS_OWNERDRAW, SS_RIGHT,
};
use windows::Win32::UI::Controls::{
    BCM_GETIDEALSIZE, BST_CHECKED, BST_UNCHECKED, EM_GETMARGINS, EM_SETCUEBANNER, PBM_SETMARQUEE,
    PBM_SETPOS, PBS_MARQUEE, PBS_SMOOTH, PROGRESS_CLASSW, TBM_GETTHUMBRECT, TBM_SETPOS,
    TBM_SETRANGEMAX, TBM_SETRANGEMIN, TBM_SETTHUMBLENGTH, TBS_BOTH, TBS_FIXEDLENGTH, TBS_HORZ,
    TBS_NOTICKS, TRACKBAR_CLASSW,
};
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_CHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON,
    BS_PUSHLIKE, CreateWindowExW, DLGC_WANTALLKEYS, DestroyWindow, ES_AUTOHSCROLL, ES_AUTOVSCROLL,
    ES_CENTER, ES_LEFT, ES_MULTILINE, ES_READONLY, ES_RIGHT, ES_WANTRETURN, GetWindowTextLengthW,
    GetWindowTextW, HMENU, MSG, SM_CXEDGE, SM_CYEDGE, SW_HIDE, SendMessageW, SetWindowTextW,
    ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CHAR, WM_GETDLGCODE, WM_KEYDOWN, WM_NCDESTROY,
    WM_SETFONT, WM_USER, WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, Result, w};

use nifty_runtime::{
    ButtonStyle, ClickHandler, DropHandler, Frame, Icon, ImageSize, LabelStyle, NumberHandler,
    Size, Surface, TextAlign, TextHandler, TextStyle, ToggleHandler, ToggleStyle,
};

use crate::context_menu::ContextMenu;
use crate::convert::{
    Scale, escape_mnemonics, handle_param, high_word, low_word, out_param, rect_size, wide,
};
use crate::draw::{
    CHECKBOX_TEXT_FORMAT, SLIDER_THUMB, button_size, checkbox_size, nav_item_size, picker_size,
    switch_size,
};
use crate::gdi::Selected;
use crate::image::Picture;
use crate::scroll::{Host, ScrollArea};
use crate::window::{Content, Control, Events, Paint, WindowState, submit};

/// How label text is measured: as written, with tabs expanded.
const LABEL_TEXT_FORMAT: DRAW_TEXT_FORMAT = DRAW_TEXT_FORMAT(DT_NOPREFIX.0 | DT_EXPANDTABS.0);

/// Text fields have no useful natural width when empty, so they start at least this wide.
const MIN_TEXT_FIELD_WIDTH: f32 = 200.0;
/// Space between a text field's border and its text, above and below.
const TEXT_FIELD_INSET: f32 = 2.0;
const DIVIDER_SIZE: Size = Size::new(1.0, 1.0);
/// A progress bar has no natural width, so it starts this wide, at this height, in DIPs.
const PROGRESS_SIZE: Size = Size::new(100.0, 15.0);
/// A slider likewise starts this wide, as tall as Fluent's slider, in DIPs.
const SLIDER_SIZE: Size = Size::new(100.0, 32.0);
/// How picker options are measured: on one line, as written.
const PICKER_TEXT_FORMAT: DRAW_TEXT_FORMAT = DRAW_TEXT_FORMAT(DT_SINGLELINE.0 | DT_NOPREFIX.0);
/// The trackbar message that answers its position, documented as `WM_USER`, which the
/// `windows` crate doesn't define.
const TBM_GETPOS: u32 = WM_USER;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WidgetKind {
    Label,
    /// A borderless read-only edit control, which is how Win32 lets users select text.
    SelectableLabel,
    Button,
    TextField,
    /// A text field this many lines tall.
    TextArea(u32),
    Checkbox,
    Switch,
    Slider,
    Picker,
    NavItem,
    Progress,
    Image,
    Divider,
}

/// Where a label's text sits, once its start and end are resolved for the reading direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Left,
    Center,
    Right,
}

impl Edge {
    fn of(align: TextAlign, right_to_left: bool) -> Self {
        match (align, right_to_left) {
            (TextAlign::Center, _) => Self::Center,
            (TextAlign::Start, false) | (TextAlign::End, true) => Self::Left,
            (TextAlign::Start, true) | (TextAlign::End, false) => Self::Right,
        }
    }
}

/// A widget: a native control, or a surface its window paints behind the controls.
#[derive(Debug)]
pub struct WinWidget(Body);

#[derive(Debug)]
enum Body {
    Control(ControlWidget),
    Surface(AreaWidget),
    ContextMenu(AreaWidget),
    DropTarget(AreaWidget),
    Scroll(ScrollWidget),
}

/// Surfaces, context menus and drop targets are kept by their host rather than being
/// windows of their own, so they cost no `HWND`: surfaces are painted behind every control,
/// context menus shown when their area is right-clicked, and drop targets take what is
/// dropped on their area.
#[derive(Debug)]
struct AreaWidget {
    id: u32,
    window: Rc<WindowState>,
    host: Host,
}

/// A scroll area, destroyed when its widget is dropped, after the event that removed it.
#[derive(Debug)]
struct ScrollWidget(Rc<ScrollArea>);

impl Drop for ScrollWidget {
    fn drop(&mut self) {
        self.0.destroy();
    }
}

impl WinWidget {
    pub(crate) fn surface(window: &Rc<WindowState>, host: Host, surface: Surface) -> Self {
        Self(Body::Surface(AreaWidget {
            id: host.surfaces(window).add(surface),
            window: Rc::clone(window),
            host,
        }))
    }

    pub(crate) fn drop_target(
        window: &Rc<WindowState>,
        host: Host,
        on_drop: DropHandler,
    ) -> Result<Self> {
        let hwnd = host.hwnd(window);
        Ok(Self(Body::DropTarget(AreaWidget {
            id: host.drop_areas(window).add(window, hwnd, on_drop)?,
            window: Rc::clone(window),
            host,
        })))
    }

    pub(crate) fn context_menu(window: &Rc<WindowState>, host: Host, menu: ContextMenu) -> Self {
        Self(Body::ContextMenu(AreaWidget {
            id: host.context_menus(window).add(Rc::new(menu)),
            window: Rc::clone(window),
            host,
        }))
    }

    pub(crate) fn scroll(area: Rc<ScrollArea>) -> Self {
        Self(Body::Scroll(ScrollWidget(area)))
    }

    /// The window this scroll area is in, and the area as the host of its content.
    ///
    /// # Panics
    /// When this isn't a scroll area, which only a runtime bug would ask.
    pub(crate) fn content(&self) -> (Rc<WindowState>, Host) {
        match &self.0 {
            Body::Scroll(scroll) => (
                Rc::clone(scroll.0.window()),
                Host::Scroll(Rc::clone(&scroll.0)),
            ),
            Body::Control(_) | Body::Surface(_) | Body::ContextMenu(_) | Body::DropTarget(_) => {
                panic!("only scroll areas hold widgets")
            }
        }
    }

    pub(crate) fn set_scroll_content(&self, size: Size) {
        if let Body::Scroll(scroll) = &self.0 {
            scroll.0.set_content(size);
        }
    }

    pub(crate) fn set_text(&self, text: &str) {
        if let Body::Control(control) = &self.0 {
            control.set_text(text);
        }
    }

    pub(crate) fn set_checked(&self, checked: bool) {
        if let Body::Control(control) = &self.0 {
            control.set_checked(checked);
        }
    }

    pub(crate) fn set_badge(&self, badge: &str) {
        if let Body::Control(control) = &self.0 {
            control.set_badge(badge);
        }
    }

    pub(crate) fn set_value(&self, value: i64) {
        if let Body::Control(control) = &self.0 {
            control.set_value(value);
        }
    }

    pub(crate) fn set_options(&self, options: &[String]) {
        if let Body::Control(control) = &self.0 {
            control.set_options(options);
        }
    }

    pub(crate) fn set_image(&self, path: Option<&Path>) {
        if let Body::Control(control) = &self.0 {
            control.set_image(path);
        }
    }

    /// Surfaces, context menus and scroll areas size from the stack they belong to, so
    /// they have no natural size.
    pub(crate) fn measure(&self) -> Size {
        match &self.0 {
            Body::Control(control) => control.measure(),
            Body::Surface(_) | Body::ContextMenu(_) | Body::DropTarget(_) | Body::Scroll(_) => {
                Size::ZERO
            }
        }
    }

    pub(crate) fn set_frame(&self, frame: Frame) {
        match &self.0 {
            Body::Control(control) => control.set_frame(frame),
            Body::Surface(surface) => {
                surface
                    .host
                    .place_surface(&surface.window, surface.id, frame);
            }
            Body::ContextMenu(menu) => {
                let rect = menu.window.scale().frame_to_pixels(frame);
                menu.host.context_menus(&menu.window).place(menu.id, rect);
            }
            Body::DropTarget(target) => {
                let rect = target.window.scale().frame_to_pixels(frame);
                let areas = target.host.drop_areas(&target.window).areas();
                areas.place(target.id, rect);
            }
            Body::Scroll(scroll) => scroll.0.place(frame),
        }
    }

    /// Takes the widget off screen now; the caller decides when to drop it.
    pub(crate) fn retire(&self) {
        match &self.0 {
            Body::Control(control) => control.retire(),
            Body::Surface(surface) => surface.host.remove_surface(&surface.window, surface.id),
            Body::ContextMenu(menu) => {
                menu.host.context_menus(&menu.window).remove(menu.id);
            }
            Body::DropTarget(target) => {
                let areas = target.host.drop_areas(&target.window).areas();
                areas.remove(target.id);
            }
            Body::Scroll(scroll) => scroll.0.retire(),
        }
    }
}

impl From<ControlWidget> for WinWidget {
    fn from(control: ControlWidget) -> Self {
        Self(Body::Control(control))
    }
}

/// Destroying the control is left to `Drop`, so a removed widget can be kept until the
/// event that removed it has finished.
pub(crate) struct ControlWidget {
    id: u16,
    kind: WidgetKind,
    control: Rc<Control>,
    window: Rc<WindowState>,
    host: Host,
}

impl fmt::Debug for ControlWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControlWidget")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("control", &self.control)
            .finish_non_exhaustive()
    }
}

/// The class, styles and behaviour a new control is created with.
struct Spec<'a> {
    kind: WidgetKind,
    class: PCWSTR,
    text: &'a str,
    style: WINDOW_STYLE,
    ex_style: WINDOW_EX_STYLE,
    font: TextStyle,
    paint: Paint,
    events: Events,
    content: Content,
}

impl ControlWidget {
    pub(crate) fn label(
        window: &Rc<WindowState>,
        host: Host,
        text: &str,
        style: LabelStyle,
    ) -> Result<Self> {
        let edge = Edge::of(style.align, window.reads_right_to_left());
        let (kind, class, class_bits) = if style.selectable {
            let align = match edge {
                Edge::Left => ES_LEFT,
                Edge::Center => ES_CENTER,
                Edge::Right => ES_RIGHT,
            };
            let bits = class_style(ES_READONLY | ES_MULTILINE | align);
            (WidgetKind::SelectableLabel, w!("EDIT"), bits)
        } else {
            let align = match edge {
                Edge::Left => SS_LEFT,
                Edge::Center => SS_CENTER,
                Edge::Right => SS_RIGHT,
            };
            (WidgetKind::Label, w!("STATIC"), SS_NOPREFIX.0 | align.0)
        };
        Self::create(
            window,
            host,
            Spec {
                kind,
                class,
                text,
                style: WINDOW_STYLE(class_bits),
                ex_style: WINDOW_EX_STYLE(0),
                font: style.font,
                paint: Paint::Text(style.color),
                events: Events::None,
                content: Content::Text,
            },
        )
    }

    /// A push button; one made with a selection is a push-like checkbox, which screen
    /// readers announce as pressed while it's selected, and which Nifty ticks itself.
    pub(crate) fn button(
        window: &Rc<WindowState>,
        host: Host,
        title: &str,
        (style, icon): (ButtonStyle, Icon),
        selected: Option<bool>,
        on_click: ClickHandler,
    ) -> Result<Self> {
        let button_style = match (style, selected) {
            (_, Some(_)) => class_style(BS_CHECKBOX | BS_PUSHLIKE),
            (ButtonStyle::Primary, None) => class_style(BS_DEFPUSHBUTTON),
            (ButtonStyle::Normal | ButtonStyle::Plain | ButtonStyle::Icon, None) => {
                class_style(BS_PUSHBUTTON)
            }
        };
        let button = Self::create(
            window,
            host,
            Spec {
                kind: WidgetKind::Button,
                class: w!("BUTTON"),
                text: title,
                style: WS_TABSTOP | WINDOW_STYLE(button_style),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Paint::Button(style, icon),
                events: Events::Click(on_click),
                content: Content::Text,
            },
        )?;
        if let Some(selected) = selected {
            button.set_checked(selected);
        }
        match style {
            ButtonStyle::Primary => window.add_primary_button(button.id),
            ButtonStyle::Icon => window.tooltips().add(window.hwnd(), button.hwnd(), title),
            ButtonStyle::Normal | ButtonStyle::Plain => {}
        }
        Ok(button)
    }

    /// A push button Nifty draws as a sidebar row; its window text is the title, so screen
    /// readers announce it as a button with that name.
    pub(crate) fn nav_item(
        window: &Rc<WindowState>,
        host: Host,
        title: &str,
        icon: Icon,
        badge: &str,
        selected: bool,
        on_click: ClickHandler,
    ) -> Result<Self> {
        Self::create(
            window,
            host,
            Spec {
                kind: WidgetKind::NavItem,
                class: w!("BUTTON"),
                text: title,
                style: WS_TABSTOP | WINDOW_STYLE(class_style(BS_PUSHBUTTON)),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Paint::NavItem(icon),
                events: Events::Click(on_click),
                content: Content::Nav {
                    selected: Cell::new(selected),
                    badge: RefCell::new(badge.to_owned()),
                },
            },
        )
    }

    /// A one-line edit control, or, `lines` lines tall, a multi-line one that wraps, takes
    /// **Return** as a new line, and scrolls. Edit controls show a placeholder on one line
    /// only, and a multi-line one types a tab on **Tab**; **Ctrl+Tab** moves on. A one-line
    /// field with `on_submit` runs it on **Return**, which it takes from the dialog manager.
    pub(crate) fn text_field(
        window: &Rc<WindowState>,
        host: Host,
        text: &str,
        (placeholder, lines): (&str, u32),
        (on_change, on_submit): (TextHandler, Option<ClickHandler>),
    ) -> Result<Self> {
        let takes_return = on_submit.is_some();
        let (kind, style) = if lines > 1 {
            (
                WidgetKind::TextArea(lines),
                WS_VSCROLL
                    | WINDOW_STYLE(class_style(ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL)),
            )
        } else {
            (
                WidgetKind::TextField,
                WINDOW_STYLE(class_style(ES_AUTOHSCROLL)),
            )
        };
        let field = Self::create(
            window,
            host,
            Spec {
                kind,
                class: w!("EDIT"),
                text,
                style: WS_TABSTOP | style,
                ex_style: WS_EX_CLIENTEDGE,
                font: TextStyle::Body,
                paint: Paint::Field,
                events: Events::Change {
                    on_change,
                    on_submit,
                },
                content: Content::Text,
            },
        )?;
        if lines > 1 {
            return Ok(field);
        }
        if takes_return {
            // SAFETY: the procedure handles only Return and its own removal, and passes
            // every other message on.
            let subclassed = unsafe {
                SetWindowSubclass(field.hwnd(), Some(submit_on_return), SUBMIT_SUBCLASS, 0)
            };
            assert!(subclassed.as_bool(), "Windows can subclass a text field");
        }
        let placeholder = wide(placeholder);
        // SAFETY: the placeholder is NUL-terminated, and the control copies it.
        unsafe {
            SendMessageW(
                field.hwnd(),
                EM_SETCUEBANNER,
                None,
                Some(LPARAM(
                    placeholder.as_ptr().expose_provenance().cast_signed(),
                )),
            );
        }
        Ok(field)
    }

    pub(crate) fn checkbox(
        window: &Rc<WindowState>,
        host: Host,
        title: &str,
        checked: bool,
        style: ToggleStyle,
        on_toggle: ToggleHandler,
    ) -> Result<Self> {
        let (kind, paint) = match style {
            ToggleStyle::Checkbox => (WidgetKind::Checkbox, Paint::Checkbox),
            ToggleStyle::Switch => (WidgetKind::Switch, Paint::Switch),
        };
        let checkbox = Self::create(
            window,
            host,
            Spec {
                kind,
                class: w!("BUTTON"),
                text: title,
                style: WS_TABSTOP | WINDOW_STYLE(class_style(BS_AUTOCHECKBOX)),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint,
                events: Events::Toggle(on_toggle),
                content: Content::Text,
            },
        )?;
        checkbox.set_checked(checked);
        Ok(checkbox)
    }

    /// The system's trackbar, with its track and thumb drawn by Nifty in Fluent's style.
    /// It reports each new whole number as the user drags it.
    pub(crate) fn slider(
        window: &Rc<WindowState>,
        host: Host,
        value: i64,
        range: (i64, i64),
        on_change: NumberHandler,
    ) -> Result<Self> {
        let slider = Self::create(
            window,
            host,
            Spec {
                kind: WidgetKind::Slider,
                class: TRACKBAR_CLASSW,
                text: "",
                style: WS_TABSTOP
                    | WINDOW_STYLE(TBS_HORZ | TBS_NOTICKS | TBS_FIXEDLENGTH | TBS_BOTH),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Paint::Slider,
                events: Events::Slide {
                    on_change,
                    last: Cell::new(value),
                },
                content: Content::Text,
            },
        )?;
        let (min, max) = range;
        // SAFETY: TBM_SETRANGEMIN and TBM_SETRANGEMAX take whether to redraw and the bound.
        unsafe {
            SendMessageW(
                slider.hwnd(),
                TBM_SETRANGEMIN,
                Some(WPARAM(0)),
                Some(int_param(min)),
            );
            SendMessageW(
                slider.hwnd(),
                TBM_SETRANGEMAX,
                Some(WPARAM(1)),
                Some(int_param(max)),
            );
        }
        slider.set_value(value);
        Ok(slider)
    }

    /// A button Nifty draws as Fluent's combo box, which shows a menu of `options` when
    /// clicked; its window text is the option it shows, so screen readers announce that.
    pub(crate) fn picker(
        window: &Rc<WindowState>,
        host: Host,
        options: &[String],
        selected: &str,
        on_pick: TextHandler,
    ) -> Result<Self> {
        let shown = if options.iter().any(|option| option == selected) {
            selected
        } else {
            ""
        };
        Self::create(
            window,
            host,
            Spec {
                kind: WidgetKind::Picker,
                class: w!("BUTTON"),
                text: shown,
                style: WS_TABSTOP | WINDOW_STYLE(class_style(BS_PUSHBUTTON)),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Paint::Picker,
                events: Events::Pick(on_pick),
                content: Content::Options(RefCell::new(options.to_vec())),
            },
        )
    }

    /// The system's progress bar, filled to `percent` out of its default range of 0 to
    /// 100, or sweeping back and forth while work goes on without one.
    pub(crate) fn progress(
        window: &Rc<WindowState>,
        host: Host,
        percent: Option<i64>,
    ) -> Result<Self> {
        let style = if percent.is_some() {
            PBS_SMOOTH
        } else {
            PBS_MARQUEE
        };
        let bar = Self::create(
            window,
            host,
            Spec {
                kind: WidgetKind::Progress,
                class: PROGRESS_CLASSW,
                text: "",
                style: WINDOW_STYLE(style),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Paint::Progress,
                events: Events::None,
                content: Content::Text,
            },
        )?;
        match percent {
            Some(percent) => bar.set_value(percent),
            // SAFETY: PBM_SETMARQUEE takes whether to animate, and the time between
            // frames, where 0 asks for the default.
            None => unsafe {
                SendMessageW(bar.hwnd(), PBM_SETMARQUEE, Some(WPARAM(1)), Some(LPARAM(0)));
            },
        }
        Ok(bar)
    }

    /// An owner-drawn static control, which its window draws the image into.
    pub(crate) fn image(
        window: &Rc<WindowState>,
        host: Host,
        path: Option<&Path>,
        size: ImageSize,
    ) -> Result<Self> {
        let image = Self::create(
            window,
            host,
            Spec {
                kind: WidgetKind::Image,
                class: w!("STATIC"),
                text: "",
                style: WINDOW_STYLE(SS_OWNERDRAW.0),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Paint::Image,
                events: Events::None,
                content: Content::Image {
                    picture: RefCell::new(None),
                    size,
                },
            },
        )?;
        image.set_image(path);
        Ok(image)
    }

    pub(crate) fn divider(window: &Rc<WindowState>, host: Host) -> Result<Self> {
        Self::create(
            window,
            host,
            Spec {
                kind: WidgetKind::Divider,
                class: w!("STATIC"),
                text: "",
                style: WINDOW_STYLE(0),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Paint::Divider,
                events: Events::None,
                content: Content::Text,
            },
        )
    }

    fn create(window: &Rc<WindowState>, host: Host, spec: Spec<'_>) -> Result<Self> {
        let id = window.allocate_id();
        let text = wide(&display_text(spec.kind, spec.text));
        // SAFETY: the parent is a live Nifty window, and `text` outlives the call. The
        // control starts at zero size until the layout places it.
        let hwnd = unsafe {
            CreateWindowExW(
                spec.ex_style,
                spec.class,
                PCWSTR(text.as_ptr()),
                WS_CHILD | WS_VISIBLE | spec.style,
                0,
                0,
                0,
                0,
                Some(host.hwnd(window)),
                Some(HMENU(std::ptr::without_provenance_mut(usize::from(id)))),
                None,
                None,
            )?
        };
        set_font(hwnd, window.font(spec.font));
        let control = Rc::new(Control {
            hwnd,
            font: spec.font,
            paint: spec.paint,
            events: spec.events,
            changing_in_code: Cell::new(false),
            content: spec.content,
        });
        window.add_control(id, Rc::clone(&control));
        Ok(Self {
            id,
            kind: spec.kind,
            control,
            window: Rc::clone(window),
            host,
        })
    }

    fn hwnd(&self) -> HWND {
        self.control.hwnd
    }

    pub(crate) fn set_text(&self, text: &str) {
        let text = self.shown(text);
        // Re-setting an unchanged value would move the cursor while the user types.
        if self.kind == WidgetKind::Divider
            || read_text(self.hwnd()) == display_text(self.kind, text)
        {
            return;
        }
        if self.is_icon_only() {
            self.window
                .tooltips()
                .update(self.window.hwnd(), self.hwnd(), text);
        }
        let text = wide(&display_text(self.kind, text));
        self.control.changing_in_code.set(true);
        // SAFETY: `text` is NUL-terminated and outlives the call.
        let result = unsafe { SetWindowTextW(self.hwnd(), PCWSTR(text.as_ptr())) };
        self.control.changing_in_code.set(false);
        result.expect("a live control can take new text");
    }

    /// What the control shows for `text`: a picker shows nothing when no option is `text`.
    fn shown<'a>(&self, text: &'a str) -> &'a str {
        match &self.control.content {
            Content::Options(options) if !options.borrow().iter().any(|option| option == text) => {
                ""
            }
            _ => text,
        }
    }

    fn set_badge(&self, badge: &str) {
        if let Content::Nav { badge: shown, .. } = &self.control.content {
            badge.clone_into(&mut shown.borrow_mut());
            self.repaint();
        }
    }

    /// Shows the image in the file at `path`, or nothing when there is none or it can't be
    /// read as an image.
    fn set_image(&self, path: Option<&Path>) {
        if let Content::Image { picture, .. } = &self.control.content {
            *picture.borrow_mut() = path.and_then(|path| Picture::read(path).ok());
            self.repaint();
        }
    }

    /// Replaces a picker's options, showing none if the one it showed is gone.
    fn set_options(&self, options: &[String]) {
        if let Content::Options(shown) = &self.control.content {
            options.clone_into(&mut shown.borrow_mut());
            self.set_text(&read_text(self.hwnd()));
        }
    }

    fn set_value(&self, value: i64) {
        match (&self.control.events, self.kind) {
            (_, WidgetKind::Progress) => {
                let position =
                    usize::try_from(value).expect("the runtime keeps percentages positive");
                // SAFETY: PBM_SETPOS takes the new position and nothing else.
                unsafe { SendMessageW(self.hwnd(), PBM_SETPOS, Some(WPARAM(position)), None) };
            }
            (Events::Slide { last, .. }, WidgetKind::Slider) => {
                last.set(value);
                // SAFETY: TBM_SETPOS takes whether to redraw and the new position, and
                // doesn't report the move.
                unsafe {
                    SendMessageW(
                        self.hwnd(),
                        TBM_SETPOS,
                        Some(WPARAM(1)),
                        Some(int_param(value)),
                    );
                }
            }
            _ => {}
        }
    }

    fn is_icon_only(&self) -> bool {
        matches!(self.control.paint, Paint::Button(ButtonStyle::Icon, _))
    }

    /// Nav items draw their selection and badge themselves, so changes need a repaint.
    fn repaint(&self) {
        // SAFETY: `hwnd` is a live child control on this thread.
        let _invalidated = unsafe { InvalidateRect(Some(self.hwnd()), None, true) };
    }

    pub(crate) fn set_checked(&self, checked: bool) {
        if let Content::Nav { selected, .. } = &self.control.content {
            if selected.replace(checked) != checked {
                self.repaint();
            }
            return;
        }
        if matches!(
            self.kind,
            WidgetKind::Checkbox | WidgetKind::Switch | WidgetKind::Button
        ) {
            let state = if checked { BST_CHECKED } else { BST_UNCHECKED };
            // SAFETY: BM_SETCHECK takes the new state and does not notify the parent.
            unsafe {
                SendMessageW(
                    self.hwnd(),
                    BM_SETCHECK,
                    Some(WPARAM(
                        usize::try_from(state.0).expect("check states are small"),
                    )),
                    None,
                );
            }
        }
    }

    /// The natural size in DIPs, measured with the control's own font at its window's DPI.
    pub(crate) fn measure(&self) -> Size {
        let scale = self.window.scale();
        match self.kind {
            WidgetKind::Divider => DIVIDER_SIZE,
            WidgetKind::Progress => PROGRESS_SIZE,
            WidgetKind::Image => {
                let Content::Image { picture, size } = &self.control.content else {
                    unreachable!("images keep their picture and size");
                };
                let natural = picture.borrow().as_ref().map_or(Size::ZERO, Picture::size);
                size.frame(natural)
            }
            WidgetKind::Button => match self.control.paint {
                Paint::Button(style, icon) => button_size(self.ideal_size(scale), style, icon),
                _ => unreachable!("buttons are painted as buttons"),
            },
            WidgetKind::NavItem => {
                let (Paint::NavItem(icon), Content::Nav { badge, .. }) =
                    (self.control.paint, &self.control.content)
                else {
                    unreachable!("nav items are painted as nav items and keep their state");
                };
                let title = self.text_size(scale, LABEL_TEXT_FORMAT);
                let badge = self.extent(scale, &badge.borrow(), LABEL_TEXT_FORMAT);
                nav_item_size(title, badge, icon)
            }
            WidgetKind::Checkbox => checkbox_size(self.text_size(scale, CHECKBOX_TEXT_FORMAT)),
            WidgetKind::Switch => switch_size(self.text_size(scale, CHECKBOX_TEXT_FORMAT)),
            WidgetKind::Slider => SLIDER_SIZE,
            WidgetKind::Picker => {
                let Content::Options(options) = &self.control.content else {
                    unreachable!("pickers keep their options");
                };
                let widest = options
                    .borrow()
                    .iter()
                    .map(|option| self.extent(scale, option, PICKER_TEXT_FORMAT))
                    .fold(
                        self.extent(scale, "", PICKER_TEXT_FORMAT),
                        |widest, size| {
                            if size.width > widest.width {
                                size
                            } else {
                                widest
                            }
                        },
                    );
                picker_size(widest)
            }
            WidgetKind::Label => self.text_size(scale, LABEL_TEXT_FORMAT),
            WidgetKind::SelectableLabel => {
                let text = self.text_size(scale, LABEL_TEXT_FORMAT);
                Size::new(text.width + self.edit_margins(scale), text.height)
            }
            WidgetKind::TextField => {
                let text = self.text_size(scale, LABEL_TEXT_FORMAT);
                let (edge_x, edge_y) = edges(scale);
                let width = text.width + self.edit_margins(scale) + 2.0 * edge_x;
                let height = text.height + 2.0 * (edge_y + TEXT_FIELD_INSET);
                Size::new(width.max(MIN_TEXT_FIELD_WIDTH), height)
            }
            WidgetKind::TextArea(lines) => {
                let line = self.extent(scale, "", LABEL_TEXT_FORMAT).height;
                let (_, edge_y) = edges(scale);
                let height = line * lines_tall(lines) + 2.0 * (edge_y + TEXT_FIELD_INSET);
                Size::new(MIN_TEXT_FIELD_WIDTH, height)
            }
        }
    }

    pub(crate) fn set_frame(&self, frame: Frame) {
        let scale = self.window.scale();
        if self.kind == WidgetKind::Slider {
            // The thumb is sized in pixels, so it follows the window's DPI here.
            let thumb = usize::try_from(scale.to_pixels(SLIDER_THUMB)).unwrap_or(0);
            // SAFETY: TBM_SETTHUMBLENGTH takes the thumb's length and nothing else.
            unsafe { SendMessageW(self.hwnd(), TBM_SETTHUMBLENGTH, Some(WPARAM(thumb)), None) };
        }
        self.host.place(self.hwnd(), scale, frame);
    }

    /// Hides the control and stops its events now.
    fn retire(&self) {
        if self.is_icon_only() {
            self.window
                .tooltips()
                .remove(self.window.hwnd(), self.hwnd());
        }
        self.window.remove_control(self.id);
        // SAFETY: `hwnd` is a live child control on this thread.
        let _was_visible = unsafe { ShowWindow(self.hwnd(), SW_HIDE) };
    }

    /// Button sizes, padding included, from Common Controls 6.
    fn ideal_size(&self, scale: Scale) -> Size {
        let mut ideal = SIZE::default();
        // SAFETY: BCM_GETIDEALSIZE writes a SIZE through the pointer it is given.
        let found = unsafe {
            SendMessageW(
                self.hwnd(),
                BCM_GETIDEALSIZE,
                None,
                Some(out_param(&mut ideal)),
            )
        };
        assert!(
            found.0 != 0,
            "BCM_GETIDEALSIZE needs Common Controls 6; embed Nifty's application manifest"
        );
        scale.size_to_dips(ideal.cx, ideal.cy)
    }

    /// The extent of the control's current text, at least one line high even when empty.
    fn text_size(&self, scale: Scale, format: DRAW_TEXT_FORMAT) -> Size {
        self.extent(scale, &read_text(self.hwnd()), format)
    }

    /// The extent of `text` in the control's font, at least one line high even when empty.
    fn extent(&self, scale: Scale, text: &str, format: DRAW_TEXT_FORMAT) -> Size {
        let mut text: Vec<u16> = text.encode_utf16().collect();
        let dc = FontContext::new(self.hwnd(), self.window.font(self.control.font));
        // DrawTextW reads the first character even when told the text is empty, and an
        // empty slice's pointer is dangling.
        if text.is_empty() {
            return scale.size_to_dips(0, dc.line_height());
        }
        let mut bounds = RECT::default();
        // SAFETY: `bounds` is a valid RECT; DT_CALCRECT only measures.
        unsafe {
            DrawTextW(dc.dc(), &mut text, &raw mut bounds, DT_CALCRECT | format);
        }
        let (width, height) = rect_size(&bounds);
        scale.size_to_dips(width, height.max(dc.line_height()))
    }

    /// The left and right space an edit control keeps around its text.
    fn edit_margins(&self, scale: Scale) -> f32 {
        // SAFETY: EM_GETMARGINS takes no parameters.
        let margins = unsafe { SendMessageW(self.hwnd(), EM_GETMARGINS, None, None) };
        let margins = margins.0.cast_unsigned();
        scale.to_dips(i32::from(low_word(margins)) + i32::from(high_word(margins)))
    }
}

impl Drop for ControlWidget {
    /// A destroyed window or scroll area has already destroyed its controls along with it.
    fn drop(&mut self) {
        if !self.host.is_destroyed(&self.window) {
            // SAFETY: the control belongs to this thread and has not been destroyed.
            unsafe { DestroyWindow(self.hwnd()) }.expect("a live control can be destroyed");
        }
    }
}

/// A control's device context with its font selected. Fields drop in order, so the font
/// is put back before the DC is released.
struct FontContext {
    _font: Selected,
    dc: ClientDc,
}

impl FontContext {
    fn new(hwnd: HWND, font: HFONT) -> Self {
        let dc = ClientDc::new(hwnd);
        Self {
            _font: Selected::new(dc.dc, font),
            dc,
        }
    }

    fn dc(&self) -> HDC {
        self.dc.dc
    }

    fn line_height(&self) -> i32 {
        let mut metrics = TEXTMETRICW::default();
        // SAFETY: `metrics` is a valid TEXTMETRICW to write into.
        let found = unsafe { GetTextMetricsW(self.dc(), &raw mut metrics) };
        assert!(
            found.as_bool(),
            "a DC with a font selected has text metrics"
        );
        metrics.tmHeight
    }
}

/// A control's device context, released on drop.
struct ClientDc {
    hwnd: HWND,
    dc: HDC,
}

impl ClientDc {
    fn new(hwnd: HWND) -> Self {
        // SAFETY: `hwnd` is a live control; its DC is released in `drop`.
        let dc = unsafe { GetDC(Some(hwnd)) };
        assert!(!dc.is_invalid(), "a live control has a device context");
        Self { hwnd, dc }
    }
}

impl Drop for ClientDc {
    fn drop(&mut self) {
        // SAFETY: releases the DC `new` acquired.
        unsafe { ReleaseDC(Some(self.hwnd), self.dc) };
    }
}

pub(crate) fn set_font(hwnd: HWND, font: HFONT) {
    // SAFETY: WM_SETFONT takes a font handle and a redraw flag; the font outlives the control.
    unsafe {
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(handle_param(font.0)),
            Some(LPARAM(1)),
        );
    }
}

/// Identifies the subclass that runs a text field's action on Return.
const SUBMIT_SUBCLASS: usize = 1;

/// A text field's subclass that takes **Return** for the field's action. The dialog
/// manager asks each control which keys it wants; a one-line edit control leaves Return to
/// the primary button, so this asks for it.
unsafe extern "system" fn submit_on_return(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass: usize,
    _data: usize,
) -> LRESULT {
    let is_return = wparam.0 == usize::from(VK_RETURN.0);
    match message {
        WM_GETDLGCODE if asks_about_return(lparam) => {
            // SAFETY: the parameters are the ones Win32 passed to this subclass.
            let code = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            LRESULT(code.0 | isize::try_from(DLGC_WANTALLKEYS).expect("dialog codes are small"))
        }
        WM_KEYDOWN if is_return => {
            submit(hwnd);
            LRESULT(0)
        }
        // The Return character that follows would make a one-line field beep.
        WM_CHAR if wparam.0 == usize::from(RETURN_CHAR) => LRESULT(0),
        WM_NCDESTROY => {
            // SAFETY: removes the subclass this procedure was installed as.
            let _removed =
                unsafe { RemoveWindowSubclass(hwnd, Some(submit_on_return), SUBMIT_SUBCLASS) };
            // SAFETY: the parameters are the ones Win32 passed to this subclass.
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        // SAFETY: the parameters are the ones Win32 passed to this subclass.
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}

/// The character a Return key press types.
const RETURN_CHAR: u8 = b'\r';

/// Whether a `WM_GETDLGCODE` asks about a Return key press, which its `LPARAM` points to.
fn asks_about_return(lparam: LPARAM) -> bool {
    // SAFETY: for WM_GETDLGCODE, `lparam` is null or points to the message being asked about.
    let message =
        unsafe { std::ptr::with_exposed_provenance::<MSG>(lparam.0.cast_unsigned()).as_ref() };
    message.is_some_and(|message| {
        message.message == WM_KEYDOWN && message.wParam.0 == usize::from(VK_RETURN.0)
    })
}

/// A slider's position.
pub(crate) fn slider_position(hwnd: HWND) -> i64 {
    // SAFETY: TBM_GETPOS takes no parameters.
    i64::try_from(unsafe { SendMessageW(hwnd, TBM_GETPOS, None, None) }.0)
        .expect("a slider's position fits an i64")
}

/// Where a slider's thumb is, in its own client area.
pub(crate) fn thumb_rect(hwnd: HWND) -> RECT {
    let mut thumb = RECT::default();
    // SAFETY: TBM_GETTHUMBRECT writes a RECT through the pointer it is given.
    unsafe { SendMessageW(hwnd, TBM_GETTHUMBRECT, None, Some(out_param(&mut thumb))) };
    thumb
}

/// A whole number as a message's `LPARAM`.
fn int_param(value: i64) -> LPARAM {
    LPARAM(isize::try_from(value).expect("Nifty targets 64-bit Windows"))
}

pub(crate) fn is_checked(hwnd: HWND) -> bool {
    // SAFETY: BM_GETCHECK takes no parameters.
    let state = unsafe { SendMessageW(hwnd, BM_GETCHECK, None, None) };
    state.0 == isize::try_from(BST_CHECKED.0).expect("check states are small")
}

/// The text an edit control holds, with its lines ending in `\n` rather than `\r\n`.
pub(crate) fn typed_text(hwnd: HWND) -> String {
    let text = read_text(hwnd);
    if text.contains("\r\n") {
        text.replace("\r\n", "\n")
    } else {
        text
    }
}

/// A text field's height in lines, as a factor.
fn lines_tall(lines: u32) -> f32 {
    f32::from(u16::try_from(lines).expect("the checker keeps text fields under 100 lines"))
}

pub(crate) fn read_text(hwnd: HWND) -> String {
    // SAFETY: `hwnd` is a live control on this thread.
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buffer = vec![0; usize::try_from(length).expect("text lengths are never negative") + 1];
    // SAFETY: `buffer` has room for the text and its NUL terminator.
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    buffer.truncate(usize::try_from(copied).expect("copied lengths are never negative"));
    String::from_utf16_lossy(&buffer)
}

/// The text as the control must hold it: `&` marks a mnemonic in button titles, and edit
/// controls break lines only at `\r\n`.
fn display_text(kind: WidgetKind, text: &str) -> Cow<'_, str> {
    match kind {
        WidgetKind::Button | WidgetKind::Checkbox | WidgetKind::Switch | WidgetKind::NavItem => {
            escape_mnemonics(text)
        }
        WidgetKind::SelectableLabel | WidgetKind::TextArea(_) if text.contains('\n') => {
            Cow::Owned(text.replace("\r\n", "\n").replace('\n', "\r\n"))
        }
        _ => Cow::Borrowed(text),
    }
}

/// The width of a sunken edge, across and down, in DIPs.
fn edges(scale: Scale) -> (f32, f32) {
    (
        scale.to_dips(system_metric(SM_CXEDGE, scale)),
        scale.to_dips(system_metric(SM_CYEDGE, scale)),
    )
}

fn system_metric(
    metric: windows::Win32::UI::WindowsAndMessaging::SYSTEM_METRICS_INDEX,
    scale: Scale,
) -> i32 {
    // SAFETY: `GetSystemMetricsForDpi` accepts every metric index.
    unsafe { GetSystemMetricsForDpi(metric, scale.dpi()) }
}

/// Win32 declares each control class's style bits in its own type; they all share the low
/// bits of the window style.
fn class_style(bits: impl Into<i64>) -> u32 {
    u32::try_from(bits.into()).expect("class styles fit the window style's low bits")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ampersands_show_literally_in_button_titles() {
        assert_eq!(
            display_text(WidgetKind::Button, "Save & Quit"),
            "Save && Quit"
        );
        assert_eq!(
            display_text(WidgetKind::Label, "Save & Quit"),
            "Save & Quit"
        );
    }

    #[test]
    fn selectable_labels_break_lines_the_way_edit_controls_do() {
        assert_eq!(
            display_text(WidgetKind::SelectableLabel, "one\ntwo\r\nthree"),
            "one\r\ntwo\r\nthree"
        );
    }

    #[test]
    fn start_and_end_follow_the_reading_direction() {
        assert_eq!(Edge::of(TextAlign::Start, false), Edge::Left);
        assert_eq!(Edge::of(TextAlign::End, false), Edge::Right);
        assert_eq!(Edge::of(TextAlign::Start, true), Edge::Right);
        assert_eq!(Edge::of(TextAlign::End, true), Edge::Left);
        assert_eq!(Edge::of(TextAlign::Center, true), Edge::Center);
    }
}
