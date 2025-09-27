use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

pub fn take_attribute(attr_name: &str, attrs: &mut Vec<syn::Attribute>) -> Option<syn::Attribute> {
    attrs
        .iter()
        .position(|attr| attr.path().is_ident(attr_name))
        .map(|pos| attrs.remove(pos))
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn unpack_generic(ty: &syn::Type, expected_type: &str, checked_value: &str) -> Box<syn::Type> {
    let type_path = match ty {
        syn::Type::Path(type_path)
        if type_path
            .path
            .segments
            .last()
            .map(|s| s.ident == expected_type)
            .unwrap_or(false)
        => type_path,
        _ => panic!("{} must be {}<T>", checked_value, expected_type),
    };
    let syn::PathArguments::AngleBracketed(args) =
        &type_path.path.segments.last().unwrap().arguments
    else {
        panic!("{} must have angle bracketed type parameters", expected_type);
    };
    match args.args.first() {
        Some(syn::GenericArgument::Type(inner_type)) => inner_type.clone().into(),
        _ => panic!("{} must have a type parameter", expected_type),
    }
}

#[allow(unused)]
pub trait DebugPrintCode {
    fn debug(self, label: Option<&str>) -> Self;
}

impl DebugPrintCode for proc_macro2::TokenStream {
    fn debug(self, label: Option<&str>) -> Self {
        match syn::parse2::<syn::File>(self.clone()) {
            Ok(file) => {
                debug_print_code_fancy(file, label);
            },
            Err(e) => {
                let label = match label {
                    Some(label) => format!(" ({})", label),
                    None => String::new(),
                };
                println!("Parse error{}: {}", label, e)
            },
        }
        self
    }
}

fn debug_print_code_fancy(file: syn::File, label: Option<&str>) {
    if let Some(label) = label {
        println!("\n=== {} ===", label);
    }
    let code = prettyplease::unparse(&file);

    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = ps.find_syntax_by_extension("rs").unwrap();
    // let mut h = HighlightLines::new(syntax, &ts.themes["base16-eighties.dark"]);
    let mut h = HighlightLines::new(syntax, &ts.themes["Solarized (dark)"]);

    for line in LinesWithEndings::from(&code) {
        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
        let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
        print!("{}", escaped);
    }
    if let Some(label) = label {
        println!("=== END {} ===\n", label);
    }
}
