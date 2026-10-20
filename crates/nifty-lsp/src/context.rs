//! What kind of code the cursor is in, worked out from the tokens before it. Code being
//! typed rarely parses, so this reads tokens rather than the syntax tree: it only needs to
//! know which blocks, calls and brackets are open and what came last.

use nifty_check::elements::{self, TrailingBlock};
use nifty_syntax::{Token, TokenKind};

use crate::document::Document;

/// What a `{ … }` holds, which decides the statements allowed in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockKind {
    TopLevel,
    /// A function body, where `return` works.
    Function,
    /// A window or view body, or a layout element's children.
    Ui,
    /// A button's action.
    Action,
    /// A menu's items.
    Menu,
    /// A struct's fields.
    Struct,
    /// A block nothing more is known about, such as one after a call that takes none.
    Other,
}

/// The call whose argument list the cursor is directly in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Call {
    /// The name called, when it is a plain name rather than a method or expression.
    pub callee: Option<String>,
    /// Positional arguments before the current one.
    pub positional: usize,
    /// Labels already given, including the current argument's.
    pub labels: Vec<String>,
    /// The current argument's label, once its `:` is typed.
    pub label: Option<String>,
}

/// Where the cursor is, and so what may be typed there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    /// The start of a statement in a block.
    Statement(BlockKind),
    /// Just after the `}` of an `if`, where `else` may follow.
    AfterIf,
    /// Anywhere an expression may start.
    Expression,
    /// The start of an argument, which may be a label or a value.
    ArgumentStart(Call),
    /// After `label:`.
    ArgumentValue(Call),
    /// After `label: .`.
    ImplicitMember(Call),
    /// After a `:` or `->` that a type follows.
    Type,
    /// Between `for item` (or `for index, item`) and `in`.
    ForIn,
    /// Where nothing can be completed: naming something new, after a value, after a `.`
    /// on a value, or in a comment or string.
    Nothing,
}

/// The cursor's context, and the call being written around it, if any.
#[derive(Debug)]
pub(crate) struct Context {
    pub slot: Slot,
    /// The innermost call whose argument list contains the cursor, even inside nested
    /// brackets, for signature help.
    pub call: Option<Call>,
}

pub(crate) fn at(document: &Document, word_start: usize) -> Context {
    let text = document.source();
    let tokens = nifty_syntax::tokens(text, document.file);
    let mut scanner = Scanner::new();
    let before = tokens
        .iter()
        .take_while(|token| token.kind != TokenKind::Eof && token.span.end <= word_start);
    for token in before {
        scanner.token(text, *token);
    }
    let inside_token = tokens
        .iter()
        .any(|token| token.span.start < word_start && word_start < token.span.end);
    let last_end = scanner.last.map_or(0, |last| last.span.end);
    let in_comment = text[last_end..word_start].contains("//");
    let slot = if inside_token || in_comment || scanner.in_string() {
        Slot::Nothing
    } else {
        scanner.slot()
    };
    Context {
        slot,
        call: scanner.innermost_call(),
    }
}

/// How many recent tokens the scanner keeps: enough for `for index, item`.
const LOOK_BACK: usize = 4;

#[derive(Debug)]
enum Frame {
    Block {
        kind: BlockKind,
        from_if: bool,
    },
    Call(Call),
    /// A function or view's parameter list.
    Params,
    /// The brackets of a list type, such as `[Todo]`.
    ListType,
    Group,
    String,
    Interpolation,
}

struct Scanner {
    frames: Vec<Frame>,
    /// Tokens other than line breaks, most recent last, up to [`LOOK_BACK`].
    recent: Vec<Token>,
    last: Option<Token>,
    last_name: Option<String>,
    /// The name scanned before `last_name`, which is an element's when the two are a
    /// top-level element and its name, as in `window settings(`.
    name_before_last: Option<String>,
    newline_after_last: bool,
    /// The block the next `{` at this depth opens, set by `fn`, `if` and the like.
    pending: Option<(BlockKind, usize, bool)>,
    /// What a `{` right after the call just closed would open.
    closed_call_block: Option<BlockKind>,
    closed_if: bool,
}

impl Scanner {
    fn new() -> Self {
        Self {
            frames: vec![Frame::Block {
                kind: BlockKind::TopLevel,
                from_if: false,
            }],
            recent: Vec::new(),
            last: None,
            last_name: None,
            name_before_last: None,
            newline_after_last: false,
            pending: None,
            closed_call_block: None,
            closed_if: false,
        }
    }

    fn token(&mut self, text: &str, token: Token) {
        if token.kind == TokenKind::Newline {
            self.newline_after_last = true;
            return;
        }
        let closed_call_block = self.closed_call_block.take();
        self.closed_if = false;
        match token.kind {
            TokenKind::Ident => {
                let name = text[token.span.range()].to_owned();
                self.name_before_last = self.last_name.replace(name);
            }
            TokenKind::Fn => self.expect_block(BlockKind::Function, false),
            TokenKind::View => self.expect_block(BlockKind::Ui, false),
            TokenKind::Struct => self.expect_block(BlockKind::Struct, false),
            TokenKind::If | TokenKind::Else | TokenKind::For => {
                let kind = self.enclosing_block();
                let from_if = token.kind != TokenKind::For;
                self.expect_block(kind, from_if);
            }
            TokenKind::LBrace => self.open_block(closed_call_block),
            TokenKind::RBrace => {
                if let Some(Frame::Block { from_if, .. }) = self.pop() {
                    self.closed_if = from_if;
                }
            }
            TokenKind::LParen => self.open_paren(),
            TokenKind::RParen => {
                if let Some(Frame::Call(call)) = self.pop() {
                    self.closed_call_block = Some(trailing_block(call.callee.as_deref()));
                }
            }
            TokenKind::LBracket => {
                let frame = if self.slot() == Slot::Type {
                    Frame::ListType
                } else {
                    Frame::Group
                };
                self.frames.push(frame);
            }
            TokenKind::RBracket | TokenKind::InterpolationEnd | TokenKind::StringEnd => {
                self.pop();
            }
            TokenKind::StringStart => self.frames.push(Frame::String),
            TokenKind::InterpolationStart => self.frames.push(Frame::Interpolation),
            TokenKind::Comma => {
                if let Some(Frame::Call(call)) = self.frames.last_mut()
                    && call.label.take().is_none()
                {
                    call.positional += 1;
                }
            }
            TokenKind::Colon => self.label_argument(),
            _ => {}
        }
        self.recent.push(token);
        if self.recent.len() > LOOK_BACK {
            self.recent.remove(0);
        }
        self.last = Some(token);
        self.newline_after_last = false;
    }

    /// Closes the innermost frame. The top level is never closed, so a stray closing
    /// bracket leaves the cursor at the top level.
    fn pop(&mut self) -> Option<Frame> {
        if self.frames.len() > 1 {
            self.frames.pop()
        } else {
            None
        }
    }

    fn expect_block(&mut self, kind: BlockKind, from_if: bool) {
        self.pending = Some((kind, self.frames.len(), from_if));
    }

    fn open_block(&mut self, closed_call_block: Option<BlockKind>) {
        let depth = self.frames.len();
        let (kind, from_if) = match self.pending.take() {
            Some((kind, at, from_if)) if at == depth => (kind, from_if),
            pending => {
                self.pending = pending;
                (closed_call_block.unwrap_or(BlockKind::Other), false)
            }
        };
        self.frames.push(Frame::Block { kind, from_if });
    }

    fn open_paren(&mut self) {
        let frame = match self.kinds().as_slice() {
            [.., TokenKind::Fn | TokenKind::View, TokenKind::Ident] => Frame::Params,
            [.., TokenKind::Ident, TokenKind::Ident]
                if self.enclosing_block() == BlockKind::TopLevel =>
            {
                Frame::Call(Call::new(self.name_before_last.clone()))
            }
            [.., TokenKind::Dot, TokenKind::Ident]
            | [.., TokenKind::RParen | TokenKind::RBracket] => Frame::Call(Call::new(None)),
            [.., TokenKind::Ident] => Frame::Call(Call::new(self.last_name())),
            _ => Frame::Group,
        };
        self.frames.push(frame);
    }

    fn label_argument(&mut self) {
        let label = match self.kinds().as_slice() {
            [.., TokenKind::LParen | TokenKind::Comma, TokenKind::Ident] => self.last_name(),
            _ => None,
        };
        if let (Some(label), Some(Frame::Call(call))) = (label, self.frames.last_mut()) {
            call.labels.push(label.clone());
            call.label = Some(label);
        }
    }

    fn enclosing_block(&self) -> BlockKind {
        match self.frames.last() {
            Some(Frame::Block { kind, .. }) => *kind,
            _ => BlockKind::Other,
        }
    }

    fn kinds(&self) -> Vec<TokenKind> {
        self.recent.iter().map(|token| token.kind).collect()
    }

    /// The name just scanned; callers check the last token is a name.
    fn last_name(&self) -> Option<String> {
        self.last_name.clone()
    }

    fn in_string(&self) -> bool {
        matches!(self.frames.last(), Some(Frame::String))
    }

    fn innermost_call(&self) -> Option<Call> {
        self.frames.iter().rev().find_map(|frame| match frame {
            Frame::Call(call) => Some(call.clone()),
            _ => None,
        })
    }

    fn slot(&self) -> Slot {
        let kinds = self.kinds();
        match self.frames.last() {
            Some(Frame::Block { kind, .. }) => self.block_slot(*kind, &kinds),
            Some(Frame::Call(call)) => call_slot(call, &kinds),
            Some(Frame::Params) => match kinds.as_slice() {
                [.., TokenKind::Colon] => Slot::Type,
                _ => Slot::Nothing,
            },
            Some(Frame::ListType) => match kinds.as_slice() {
                [.., TokenKind::LBracket] => Slot::Type,
                _ => Slot::Nothing,
            },
            Some(Frame::Group | Frame::Interpolation) => expression_slot(&kinds),
            Some(Frame::String) | None => Slot::Nothing,
        }
    }

    fn block_slot(&self, kind: BlockKind, kinds: &[TokenKind]) -> Slot {
        if kind == BlockKind::Struct {
            return match kinds {
                [.., TokenKind::Colon] => Slot::Type,
                _ => Slot::Nothing,
            };
        }
        let at_line_start = self.newline_after_last;
        match kinds {
            [] | [.., TokenKind::LBrace] => Slot::Statement(kind),
            _ if at_line_start => Slot::Statement(kind),
            [.., TokenKind::RBrace] if self.closed_if => Slot::AfterIf,
            [.., TokenKind::Colon | TokenKind::Arrow] => Slot::Type,
            [.., TokenKind::For, TokenKind::Ident]
            | [
                ..,
                TokenKind::For,
                TokenKind::Ident,
                TokenKind::Comma,
                TokenKind::Ident,
            ] => Slot::ForIn,
            [.., TokenKind::For, TokenKind::Ident, TokenKind::Comma] => Slot::Nothing,
            _ => expression_slot(kinds),
        }
    }
}

fn call_slot(call: &Call, kinds: &[TokenKind]) -> Slot {
    match kinds {
        [.., TokenKind::LParen | TokenKind::Comma] => Slot::ArgumentStart(call.clone()),
        [.., TokenKind::Colon] if call.label.is_some() => Slot::ArgumentValue(call.clone()),
        [.., TokenKind::Colon, TokenKind::Dot] if call.label.is_some() => {
            Slot::ImplicitMember(call.clone())
        }
        _ => expression_slot(kinds),
    }
}

/// After an operator or opening bracket an expression may start; after a value, a name
/// being declared or a member `.`, nothing from the completion list fits.
fn expression_slot(kinds: &[TokenKind]) -> Slot {
    match kinds.last() {
        Some(&kind) if ends_value(kind) || declares_name(kind) || kind == TokenKind::Dot => {
            Slot::Nothing
        }
        None | Some(_) => Slot::Expression,
    }
}

fn ends_value(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Ident
            | TokenKind::Int
            | TokenKind::Float
            | TokenKind::True
            | TokenKind::False
            | TokenKind::StringEnd
            | TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::RBrace
    )
}

fn declares_name(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::State
            | TokenKind::Let
            | TokenKind::Fn
            | TokenKind::View
            | TokenKind::Struct
            | TokenKind::For
    )
}

/// What the block after a call to `callee` holds.
fn trailing_block(callee: Option<&str>) -> BlockKind {
    match callee
        .and_then(elements::element)
        .map(|element| element.block)
    {
        Some(TrailingBlock::Children) => BlockKind::Ui,
        Some(TrailingBlock::Action | TrailingBlock::OptionalAction) => BlockKind::Action,
        Some(TrailingBlock::Menu) => BlockKind::Menu,
        Some(TrailingBlock::None) | None => BlockKind::Other,
    }
}

impl Call {
    fn new(callee: Option<String>) -> Self {
        Self {
            callee,
            positional: 0,
            labels: Vec::new(),
            label: None,
        }
    }
}
