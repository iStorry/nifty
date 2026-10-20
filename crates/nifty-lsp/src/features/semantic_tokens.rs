//! Semantic tokens for names, which the `TextMate` grammar can't tell apart: a struct from a
//! view, a state value from a local, a built-in element from a declared function. Keywords,
//! literals, operators and comments are left to the grammar.

use lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend};

use nifty_check::hir::Type;
use nifty_check::{elements, functions, methods};

use nifty_syntax::FileId;

use crate::document::Encoding;
use crate::index::{Index, Role, SymbolKind, Unresolved};
use crate::project::Project;

#[derive(Clone, Copy)]
enum TokenType {
    Struct,
    View,
    Function,
    Method,
    Variable,
    Parameter,
    Property,
    EnumMember,
    Type,
}

impl TokenType {
    const ALL: [Self; 9] = [
        Self::Struct,
        Self::View,
        Self::Function,
        Self::Method,
        Self::Variable,
        Self::Parameter,
        Self::Property,
        Self::EnumMember,
        Self::Type,
    ];

    fn lsp(self) -> SemanticTokenType {
        match self {
            Self::Struct => SemanticTokenType::STRUCT,
            // Views are used like components, which editors theme as classes.
            Self::View => SemanticTokenType::CLASS,
            Self::Function => SemanticTokenType::FUNCTION,
            Self::Method => SemanticTokenType::METHOD,
            Self::Variable => SemanticTokenType::VARIABLE,
            Self::Parameter => SemanticTokenType::PARAMETER,
            Self::Property => SemanticTokenType::PROPERTY,
            Self::EnumMember => SemanticTokenType::ENUM_MEMBER,
            Self::Type => SemanticTokenType::TYPE,
        }
    }
}

#[derive(Clone, Copy)]
enum Modifier {
    Declaration,
    Readonly,
    DefaultLibrary,
}

impl Modifier {
    const ALL: [Self; 3] = [Self::Declaration, Self::Readonly, Self::DefaultLibrary];

    fn lsp(self) -> SemanticTokenModifier {
        match self {
            Self::Declaration => SemanticTokenModifier::DECLARATION,
            Self::Readonly => SemanticTokenModifier::READONLY,
            Self::DefaultLibrary => SemanticTokenModifier::DEFAULT_LIBRARY,
        }
    }

    fn bit(self) -> u32 {
        1 << self as u32
    }
}

pub(crate) fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TokenType::ALL.map(TokenType::lsp).to_vec(),
        token_modifiers: Modifier::ALL.map(Modifier::lsp).to_vec(),
    }
}

/// Every name in `file`, encoded relative to the one before it as the protocol asks.
pub(crate) fn semantic_tokens(
    project: &Project,
    file: FileId,
    encoding: Encoding,
) -> Vec<SemanticToken> {
    let document = project.document(file);
    let mut previous_line = 0;
    let mut previous_start = 0;
    project
        .index
        .occurrences_in(file)
        .iter()
        .map(|occurrence| {
            let start = document.position(occurrence.span.start, encoding);
            let delta_line = start.line - previous_line;
            let delta_start = if delta_line == 0 {
                start.character - previous_start
            } else {
                start.character
            };
            previous_line = start.line;
            previous_start = start.character;

            let (token_type, modifiers) = classify(
                &project.index,
                occurrence.role,
                document.text(occurrence.span),
            );
            SemanticToken {
                delta_line,
                delta_start,
                length: encoding.len(document.text(occurrence.span)),
                token_type: token_type as u32,
                token_modifiers_bitset: modifiers,
            }
        })
        .collect()
}

fn classify(index: &Index, role: Role, name: &str) -> (TokenType, u32) {
    match role {
        Role::Declaration(id) => {
            let (token_type, modifiers) = classify_symbol(index.symbol(id).kind);
            (token_type, modifiers | Modifier::Declaration.bit())
        }
        Role::Reference(id) => classify_symbol(index.symbol(id).kind),
        Role::Unresolved(unresolved) => match unresolved {
            Unresolved::Function => (
                TokenType::Function,
                built_in(elements::element(name).is_some() || functions::function(name).is_some()),
            ),
            Unresolved::Type => (TokenType::Type, built_in(Type::primitive(name).is_some())),
            Unresolved::Variable => (TokenType::Variable, 0),
            Unresolved::Field => (TokenType::Property, 0),
            Unresolved::Method => (TokenType::Method, built_in(methods::method(name).is_some())),
            Unresolved::ImplicitMember => (TokenType::EnumMember, 0),
        },
    }
}

/// The modifier for a name the file doesn't declare: from the built-in tables, or none
/// for a misspelling.
fn built_in(is_built_in: bool) -> u32 {
    if is_built_in {
        Modifier::DefaultLibrary.bit()
    } else {
        0
    }
}

/// A symbol's token type, marked readonly when the name can't be assigned.
fn classify_symbol(kind: SymbolKind) -> (TokenType, u32) {
    let readonly = Modifier::Readonly.bit();
    match kind {
        SymbolKind::State => (TokenType::Variable, 0),
        SymbolKind::Local | SymbolKind::Window => (TokenType::Variable, readonly),
        SymbolKind::Parameter => (TokenType::Parameter, readonly),
        SymbolKind::Struct => (TokenType::Struct, 0),
        SymbolKind::Field => (TokenType::Property, 0),
        SymbolKind::Function => (TokenType::Function, 0),
        SymbolKind::View => (TokenType::View, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_names_relative_to_the_previous_one() {
        let project = Project::single("state n = 0\nlet m = n");
        let tokens: Vec<_> = semantic_tokens(&project, FileId::default(), Encoding::Utf16)
            .into_iter()
            .map(|t| {
                (
                    t.delta_line,
                    t.delta_start,
                    t.length,
                    t.token_type,
                    t.token_modifiers_bitset,
                )
            })
            .collect();
        let variable = TokenType::Variable as u32;
        let declaration = Modifier::Declaration.bit();
        let readonly = Modifier::Readonly.bit();
        assert_eq!(
            tokens,
            [
                (0, 6, 1, variable, declaration),
                (1, 4, 1, variable, declaration | readonly),
                (0, 4, 1, variable, 0),
            ]
        );
    }

    #[test]
    fn only_names_in_the_built_in_tables_are_marked_default_library() {
        let source = "fn f(xs: [int], t: strng) {\n  xs.push(1)\n  xs.pushh(1)\n  download(\"u\")\n  downlod(\"u\")\n}";
        let project = Project::single(source);
        let marked: Vec<_> = project
            .index
            .occurrences_in(FileId::default())
            .iter()
            .filter_map(|occurrence| {
                let name = project.text(occurrence.span);
                let (_, modifiers) = classify(&project.index, occurrence.role, name);
                (modifiers & Modifier::DefaultLibrary.bit() != 0).then_some(name)
            })
            .collect();
        assert_eq!(marked, ["int", "push", "download"]);
    }
}
