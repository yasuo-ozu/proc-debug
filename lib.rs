use argp::FromArgs;
use bat::PrettyPrinter;
pub use proc_debug_macro::proc_debug;
use proc_macro2::{TokenStream, TokenTree};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::{io::Write, str::FromStr};
use syn::*;
use template_quote::{quote, quote_spanned, ToTokens};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

const COUNTER: proc_state::Global<Mutex<usize>> = proc_state::new!();

fn print<R>(f: impl FnOnce(&mut StandardStream) -> R) -> R {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    stdout
        .set_color(
            ColorSpec::new()
                .set_bg(Some(Color::Cyan))
                .set_fg(Some(Color::Black))
                .set_bold(true),
        )
        .unwrap();
    f(&mut stdout)
}

enum MacroOutput {
    Expr(Expr),
    Type(Type),
    ImplItem(Vec<ImplItem>),
    TraitItem(Vec<TraitItem>),
    ForeignItem(Vec<ForeignItem>),
    Item(Vec<Item>),
    Stmt(Vec<Stmt>),
    Other(TokenStream),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum MacroKind {
    Function,
    Attribute,
    Derive,
}

impl FromStr for MacroKind {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "function" => Ok(Self::Function),
            "attribute" => Ok(Self::Attribute),
            "derive" => Ok(Self::Derive),
            _ => Err("Bad name"),
        }
    }
}

impl ToTokens for MacroOutput {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let rhs = match self {
            MacroOutput::Expr(expr) => quote!(#expr),
            MacroOutput::Type(ty) => quote!(#ty),
            MacroOutput::ImplItem(o) => quote!(#(#o)*),
            MacroOutput::ForeignItem(o) => quote!(#(#o)*),
            MacroOutput::TraitItem(o) => quote!(#(#o)*),
            MacroOutput::Item(o) => quote!(#(#o)*),
            MacroOutput::Stmt(o) => quote!(#(#o)*),
            MacroOutput::Other(o) => o.clone(),
        };
        tokens.extend(rhs);
    }
}

fn simplify_and_replace(tokens: TokenStream, depth: usize) -> TokenStream {
    let mut out = TokenStream::new();
    if depth == 0 {
        out.extend(quote!(__proc_debug_ellipsis! {}));
        return out;
    }
    let mut count = 0;
    for token in tokens {
        match &token {
            TokenTree::Group(g) => {
                let inner = simplify_and_replace(g.stream(), depth - 1);
                out.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                    g.delimiter(),
                    inner,
                ))));
            }
            TokenTree::Punct(p) if p.as_char() == ';' => {
                count += 1;
                out.extend(Some(token.clone()));
                if count >= depth {
                    out.extend(quote_spanned!(p.span() => __proc_debug_ellipsis!{}));
                    break;
                }
            }
            TokenTree::Ident(ident) if &ident.to_string() == "$crate" => {
                out.extend(quote_spanned!(ident.span() => __proc_debug_dollar_crate!{}));
            }
            _ => {
                out.extend(Some(token));
            }
        }
    }
    out
}

fn unreplace(tokens: TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    let mut tokens: VecDeque<_> = tokens.into_iter().collect();
    while let Some(token) = tokens.pop_front() {
        if let TokenTree::Ident(ident) = token.clone() {
            match tokens.pop_front() {
                Some(TokenTree::Punct(p)) if p.as_char() == '!' => {
                    match tokens.pop_front() {
                        Some(TokenTree::Group(g))
                            if g.delimiter() == proc_macro2::Delimiter::Brace =>
                        {
                            match ident.to_string().as_str() {
                                "__proc_debug_ellipsis" => {
                                    out.extend(quote_spanned!(ident.span() => ...));
                                    continue;
                                }
                                "__proc_debug_dollar_crate" => {
                                    out.extend(quote_spanned!(ident.span() => $crate));
                                    continue;
                                }
                                _ => (),
                            }
                            tokens.push_front(TokenTree::Group(g));
                        }
                        Some(o) => tokens.push_front(o),
                        None => (),
                    }
                    tokens.push_front(TokenTree::Punct(p));
                }
                Some(o) => tokens.push_front(o),
                None => (),
            }
        }
        if let TokenTree::Group(g) = token.clone() {
            let ng = proc_macro2::Group::new(g.delimiter(), unreplace(g.stream()));
            out.extend(Some(TokenTree::Group(ng)));
        } else {
            out.extend(Some(token));
        }
    }
    out
}

impl MacroOutput {
    fn from_tokens(tokens: TokenStream, kind: &MacroKind) -> Self {
        struct Sequentary<T>(Vec<T>);
        impl<T> syn::parse::Parse for Sequentary<T>
        where
            T: syn::parse::Parse,
        {
            fn parse(input: parse::ParseStream) -> Result<Self> {
                let mut v = Vec::new();
                while !input.is_empty() {
                    v.push(input.parse()?)
                }
                Ok(Self(v))
            }
        }
        if kind == &MacroKind::Function {
            if let Ok(ident) = parse2::<Ident>(tokens.clone()) {
                if ident.to_string().chars().next().unwrap().is_uppercase() {
                    return Self::Type(parse_quote! {#ident});
                } else {
                    return Self::Expr(parse_quote!(#ident));
                }
            }
            if let Ok(ty) = parse2::<Type>(tokens.clone()) {
                return Self::Type(ty);
            }
        }
        if let Ok(s) = parse2::<Sequentary<_>>(tokens.clone()) {
            return Self::ImplItem(s.0);
        }
        if let Ok(s) = parse2::<Sequentary<_>>(tokens.clone()) {
            return Self::TraitItem(s.0);
        }
        if let Ok(s) = parse2::<Sequentary<_>>(tokens.clone()) {
            return Self::ForeignItem(s.0);
        }
        if let Ok(s) = parse2::<Sequentary<_>>(tokens.clone()) {
            return Self::Item(s.0);
        }
        if let Ok(s) = parse2::<Sequentary<_>>(tokens.clone()) {
            return Self::Stmt(s.0);
        }
        Self::Other(tokens)
    }

    fn emit(&self) -> TokenStream {
        match self {
            MacroOutput::Expr(expr) => quote! {#expr},
            MacroOutput::Type(ty) => quote! {#ty},
            o => quote! {#o},
        }
    }
}

fn show_macro_call(
    modpath: &str,
    macro_name: &str,
    file: &str,
    line: usize,
    macro_kind: &str,
    macro_inputs: &[String],
) {
    let content = match macro_kind {
        "function" => format!("{macro_name}!{{{}}}", macro_inputs[0]),
        "attribute" => format!(
            "#[{}({})]\n{}",
            macro_name, macro_inputs[0], macro_inputs[1]
        ),
        "derive" => format!("#[derive({})]\n{}", macro_inputs[0], macro_inputs[1]),
        _ => format!("{}", macro_inputs.join(",")),
    };
    let content = content
        .split("\n")
        .map(|s| format!("  {}", s))
        .collect::<Vec<_>>()
        .join("\n");
    print(|out| writeln!(out, "👉 input of {modpath}::{macro_name} ({file}:{line})",)).unwrap();
    PrettyPrinter::new()
        .input_from_reader(content.as_bytes())
        .language("rust")
        .print()
        .unwrap();
    writeln!(std::io::stdout(), "",).unwrap();
}

pub fn show_macro_output(
    modpath: &str,
    macro_name: &str,
    file: &str,
    line: usize,
    macro_output: &str,
) {
    print(|out| writeln!(out, "👉 output of {modpath}::{macro_name} ({file}:{line})",)).unwrap();
    let content = macro_output
        .split("\n")
        .map(|s| format!("  {}", s))
        .collect::<Vec<_>>()
        .join("\n");
    PrettyPrinter::new()
        .input_from_bytes(content.as_bytes())
        .language("rust")
        .print()
        .unwrap();
    writeln!(std::io::stdout(), "",).unwrap();
}

/// Input for `proc-debug`
#[derive(FromArgs)]
struct ProcDebugArgs {
    /// debug all macros
    #[argp(switch, short = 'a')]
    all: bool,
    /// hide outputs match
    #[argp(option, short = 'n')]
    not: Vec<String>,
    /// full or partial path of macro definition
    #[argp(option, short = 'p')]
    path: Vec<String>,
    /// search queries to show debug
    #[argp(positional, greedy)]
    queries: Vec<String>,
    /// depth to show in macro output
    #[argp(option, short = 'd')]
    depth: Option<usize>,
    /// count to show in display
    #[argp(option, short = 'c')]
    count: Option<usize>,
    /// verbose
    #[argp(switch, short = 'v')]
    verbose: bool,
}

impl ProcDebugArgs {
    fn from_env() -> Option<Self> {
        let flags = std::env::var("PROC_DEBUG_FLAGS").ok()?;
        let flags = shtring::split(&flags).ok()?;
        Some(
            ProcDebugArgs::from_args(&["proc-debug"], &flags).unwrap_or_else(|early_exit| {
                let mut stderr = StandardStream::stderr(ColorChoice::Always);
                stderr
                    .set_color(
                        ColorSpec::new()
                            .set_bg(Some(Color::Yellow))
                            .set_fg(Some(Color::Black))
                            .set_bold(true),
                    )
                    .unwrap();
                match early_exit {
                    argp::EarlyExit::Help(help) => {
                        writeln!(&mut stderr, "{}", help.generate_default()).unwrap()
                    }
                    argp::EarlyExit::Err(err) => writeln!(
                        &mut stderr,
                        "{} \n\n Set PROC_DEBUG_FLAGS=\"--help\" for more information.",
                        err
                    )
                    .unwrap(),
                }
                std::process::exit(1)
            }),
        )
    }
}

#[allow(unused)]
struct Entry<'a> {
    label: &'a str,
    file: &'a str,
    line: usize,
    modpath: &'a str,
    macro_kind: &'a str,
    macro_name: &'a str,
    macro_inputs: &'a [String],
}

impl<'a> Entry<'a> {
    fn check_filter(&self, args: &ProcDebugArgs, n: usize) -> bool {
        let content = [&self.label, &self.file, &self.modpath, &self.macro_name];
        let pattern = format!("{}::{}", &self.modpath, &self.macro_name);

        if n > args.count.unwrap_or(usize::MAX) {
            return false;
        }
        if args.all {
            return true;
        }
        if content
            .iter()
            .any(|s| args.not.iter().any(|t| s.contains(t)))
        {
            return false;
        }
        if args.path.iter().any(|m| {
            m == &pattern
                || pattern.starts_with(&format!("{}::", m))
                || pattern.ends_with(&format!("::{}", m))
        }) {
            return true;
        }
        if content
            .iter()
            .any(|s| args.queries.iter().any(|t| s.contains(t)))
        {
            return true;
        }
        false
    }
}

fn count() -> usize {
    let ctr = COUNTER.or_insert(Mutex::new(0));
    let mut n = ctr.lock().unwrap();
    *n += 1;
    *n
}

#[doc(hidden)]
pub fn proc_wrapper<F: FnOnce() -> TokenStream>(
    label: &str,
    file: &str,
    line: usize,
    modpath: &str,
    macro_kind: &str,
    macro_name: &str,
    macro_inputs: &[String],
    f: F,
) -> TokenStream {
    let entry = Entry {
        label,
        file,
        line,
        modpath,
        macro_kind,
        macro_name,
        macro_inputs,
    };
    let n = count();
    let ret = f();
    if let Some(args) = ProcDebugArgs::from_env() {
        if entry.check_filter(&args, n) {
            show_macro_call(modpath, macro_name, file, line, macro_kind, macro_inputs);
            let tokens: TokenStream = ret.into();
            let output =
                MacroOutput::from_tokens(tokens.clone(), &MacroKind::from_str(macro_kind).unwrap());
            let simplified = simplify_and_replace(
                tokens,
                if args.verbose {
                    usize::MAX
                } else {
                    args.depth.unwrap_or(4)
                },
            );

            show_macro_output(
                modpath,
                macro_name,
                file,
                line,
                &unreplace(simplified).to_string(),
            );
            output.emit().into()
        } else {
            ret
        }
    } else {
        ret
    }
}
