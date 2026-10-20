//! Turns a program descriptor's windows and views into the runtime's templates, checking
//! each template has what its kind needs.

use nifty_runtime::{
    Size, StateId, StateSet, Template, Templates, ToggleStyle, Widget, WindowTemplate,
};

use crate::context::AppError;
use crate::ffi::{Slice, Str};
use crate::logic::{Binding, Compiled, Frame, Region, ViewTemplate};
use crate::program::{ABI_VERSION, BindingDesc, ProgramDesc, TemplateDesc, TemplateKind};

pub(crate) struct Lowered {
    pub windows: Vec<WindowTemplate<Compiled>>,
    /// Indexed by view id.
    pub views: Vec<ViewTemplate>,
}

pub(crate) fn lower(program: &'static ProgramDesc) -> Result<Lowered, AppError> {
    if program.abi_version != ABI_VERSION {
        return Err(AppError::malformed(&format!(
            "it was built for runtime version {}, not {ABI_VERSION}",
            program.abi_version
        )));
    }
    let lowerer = Lowerer {
        states: program.states.items().len(),
    };
    let windows = program
        .windows
        .items()
        .iter()
        .map(|window| {
            Ok(WindowTemplate {
                title: text(&window.title)?.to_owned(),
                size: Size::new(window.width, window.height),
                backdrop: window.backdrop,
                style: window.style,
                children: lowerer.templates(&window.children)?,
                frame: Frame::empty(&window.frame),
            })
        })
        .collect::<Result<_, AppError>>()?;
    let views = program
        .views
        .items()
        .iter()
        .map(|view| {
            Ok(ViewTemplate {
                body: lowerer.templates(&view.body)?,
                frame: &view.frame,
            })
        })
        .collect::<Result<_, AppError>>()?;
    Ok(Lowered { windows, views })
}

struct Lowerer {
    /// How many states the program declares, which every dependency must be one of.
    states: usize,
}

impl Lowerer {
    fn templates(
        &self,
        list: &'static Slice<TemplateDesc>,
    ) -> Result<Templates<Compiled>, AppError> {
        list.items()
            .iter()
            .map(|desc| self.template(desc))
            .collect()
    }

    fn template(&self, desc: &'static TemplateDesc) -> Result<Template<Compiled>, AppError> {
        let deps = self.deps(&desc.deps)?;
        let widget = match desc.kind {
            TemplateKind::Stack | TemplateKind::Scroll => {
                return Ok(Template::Stack {
                    style: desc.stack,
                    children: self.templates(&desc.children)?,
                    scrolls: desc.kind == TemplateKind::Scroll,
                });
            }
            TemplateKind::Region => {
                let region = Region {
                    expand: required(desc.region.expand, "a region's expansion")?,
                    lists: desc
                        .region
                        .lists
                        .items()
                        .iter()
                        .map(|list| self.templates(list))
                        .collect::<Result<_, _>>()?,
                };
                return Ok(Template::Region { region, deps });
            }
            TemplateKind::Label => Widget::Label {
                text: required(desc.text, "a label's text")?,
                style: desc.label,
            },
            TemplateKind::Button => Widget::Button {
                title: required(desc.text, "a button's title")?,
                style: desc.button,
                icon: desc.icon,
                selected: desc.value,
                action: required(desc.action, "a button's action")?,
            },
            TemplateKind::TextField => Widget::TextField {
                value: binding(desc.binding, "a text field's binding")?,
                placeholder: text(&desc.placeholder)?.to_owned(),
            },
            TemplateKind::Checkbox | TemplateKind::Switch => Widget::Checkbox {
                checked: binding(desc.binding, "a checkbox's binding")?,
                title: desc.text,
                style: if desc.kind == TemplateKind::Switch {
                    ToggleStyle::Switch
                } else {
                    ToggleStyle::Checkbox
                },
            },
            TemplateKind::Slider => Widget::Slider {
                value: binding(desc.binding, "a slider's binding")?,
                min: desc.min,
                max: desc.max,
            },
            TemplateKind::Picker => Widget::Picker {
                selection: binding(desc.binding, "a picker's binding")?,
                options: required(desc.value, "a picker's options")?,
            },
            TemplateKind::NavItem => Widget::NavItem {
                title: required(desc.text, "a nav item's title")?,
                value: required(desc.value, "a nav item's value")?,
                selection: binding(desc.binding, "a nav item's selection")?,
                icon: desc.icon,
                badge: desc.badge,
            },
            TemplateKind::Progress => Widget::Progress { value: desc.value },
            TemplateKind::Spacer => Widget::Spacer,
            TemplateKind::Divider => Widget::Divider,
        };
        Ok(Template::Widget { widget, deps })
    }

    fn deps(&self, deps: &Slice<u32>) -> Result<StateSet, AppError> {
        let mut set = StateSet::default();
        for state in deps.items() {
            let state = *state as usize;
            if state >= self.states {
                return Err(AppError::malformed(&format!(
                    "UI depends on state {state}, but there are only {}",
                    self.states
                )));
            }
            set.insert(StateId(state));
        }
        Ok(set)
    }
}

fn required<T>(part: Option<T>, what: &str) -> Result<T, AppError> {
    part.ok_or_else(|| AppError::malformed(&format!("{what} is missing")))
}

fn binding(binding: BindingDesc, what: &str) -> Result<Binding, AppError> {
    match (binding.read, binding.write) {
        (Some(read), Some(write)) => Ok(Binding { read, write }),
        _ => Err(AppError::malformed(&format!("{what} is missing"))),
    }
}

fn text(text: &Str) -> Result<&'static str, AppError> {
    text.text()
        .map_err(|error| AppError::malformed(&format!("some text isn't UTF-8: {error}")))
}
