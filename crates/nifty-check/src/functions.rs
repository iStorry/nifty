//! The built-in functions and the arguments each one takes. The checker dispatches on
//! [`Function::kind`], and tools such as the language server read the same table.

use crate::elements::{BoundType, Param, ParamKind, named, positional, required};

/// A built-in function such as `download`, called from functions and button actions.
#[derive(Debug)]
pub struct Function {
    pub name: &'static str,
    pub kind: FunctionKind,
    pub params: &'static [Param],
    pub summary: &'static str,
}

/// Which function a table entry is, for the checker to dispatch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionKind {
    Download,
    PickFile,
    PickFiles,
    PickFolder,
    PickSavePath,
    OpenWindow,
    CloseWindow,
}

/// Saves a file from the web in the background, reporting into two bound states.
pub const DOWNLOAD: &str = "download";

/// Opens a window by its name.
pub const OPEN_WINDOW: &str = "open_window";

/// The settings file dialogs share: the text shown above the files, the extensions
/// allowed, and the folder they open in.
pub const TITLE: &str = "title";
pub const TYPES: &str = "types";
pub const FROM: &str = "from";
/// The file name a save dialog suggests.
pub const NAME: &str = "name";

const TITLE_PARAM: Param = named(TITLE, ParamKind::Text);
const TYPES_PARAM: Param = named(TYPES, ParamKind::TextList);
const FROM_PARAM: Param = named(FROM, ParamKind::Text);
const WINDOW_PARAM: Param = positional("window", ParamKind::Window);

pub const FUNCTIONS: &[Function] = &[
    Function {
        name: DOWNLOAD,
        kind: FunctionKind::Download,
        params: &[
            positional("url", ParamKind::Text),
            required("to", ParamKind::Text),
            required("progress", ParamKind::Binding(BoundType::Int)),
            required("status", ParamKind::Binding(BoundType::String)),
        ],
        summary: "Saves the file at a web address into a folder, in the background.",
    },
    Function {
        name: "pick_file",
        kind: FunctionKind::PickFile,
        params: &[
            positional("path", ParamKind::Binding(BoundType::String)),
            TITLE_PARAM,
            TYPES_PARAM,
            FROM_PARAM,
        ],
        summary: "Asks the user to choose a file, and writes its path into the bound state.",
    },
    Function {
        name: "pick_files",
        kind: FunctionKind::PickFiles,
        params: &[
            positional("paths", ParamKind::Binding(BoundType::Strings)),
            TITLE_PARAM,
            TYPES_PARAM,
            FROM_PARAM,
        ],
        summary: "Asks the user to choose files, and writes their paths into the bound list.",
    },
    Function {
        name: "pick_folder",
        kind: FunctionKind::PickFolder,
        params: &[
            positional("folder", ParamKind::Binding(BoundType::String)),
            TITLE_PARAM,
            FROM_PARAM,
        ],
        summary: "Asks the user to choose a folder, and writes its path into the bound state.",
    },
    Function {
        name: "pick_save_path",
        kind: FunctionKind::PickSavePath,
        params: &[
            positional("path", ParamKind::Binding(BoundType::String)),
            TITLE_PARAM,
            TYPES_PARAM,
            named(NAME, ParamKind::Text),
            FROM_PARAM,
        ],
        summary: "Asks the user where to save a file, and writes that path into the bound state.",
    },
    Function {
        name: OPEN_WINDOW,
        kind: FunctionKind::OpenWindow,
        params: &[WINDOW_PARAM],
        summary: "Opens the window with this name, or brings it to the front if it's open.",
    },
    Function {
        name: "close_window",
        kind: FunctionKind::CloseWindow,
        params: &[WINDOW_PARAM],
        summary: "Closes the window with this name, if it's open.",
    },
];

/// The function called `name`, if it is built in.
#[must_use]
pub fn function(name: &str) -> Option<&'static Function> {
    FUNCTIONS.iter().find(|function| function.name == name)
}
