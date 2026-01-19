use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
    util::{LinesWithEndings, as_24_bit_terminal_escaped},
};

pub fn take_attribute(attr_name: &str, attrs: &mut Vec<syn::Attribute>) -> Option<syn::Attribute> {
    attrs
        .iter()
        .position(|attr| attr.path().is_ident(attr_name))
        .map(|pos| attrs.remove(pos))
}

#[allow(dead_code)]
pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn unpack_generic(
    ty: &syn::Type,
    expected_types: &[&str],
    checked_value: &str,
) -> Box<syn::Type> {
    let mut current_type = ty;

    for expected_type in expected_types {
        let type_path = match current_type {
            syn::Type::Path(type_path)
                if type_path
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident == expected_type)
                    .unwrap_or(false) =>
            {
                type_path
            }
            _ => panic!(
                "{} must be wrapped in {}",
                checked_value,
                expected_types.join("<") + &">".repeat(expected_types.len())
            ),
        };

        let syn::PathArguments::AngleBracketed(args) =
            &type_path.path.segments.last().unwrap().arguments
        else {
            panic!(
                "{} must have angle bracketed type parameters",
                expected_type
            );
        };

        current_type = match args.args.first() {
            Some(syn::GenericArgument::Type(inner_type)) => inner_type,
            _ => panic!("{} must have a type parameter", expected_type),
        };
    }

    current_type.clone().into()
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
            }
            Err(e) => {
                let label = match label {
                    Some(label) => format!(" ({})", label),
                    None => String::new(),
                };
                println!("Parse error{}: {}", label, e);
                println!("{}", self.clone().to_string());
            }
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
