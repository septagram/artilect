use dioxus::prelude::*;

mod element_constructors;
mod system_prompt;
pub use system_prompt::SystemPrompt;

pub fn get_artilect_name() -> String {
    let name =  std::env::var("NAME")
        .expect("NAME must be set")
        .trim()
        .to_string();

    if name.is_empty() {
        panic!("NAME cannot be empty");
    }

    name
}

pub fn render_prompt(content: Element) -> String {
    let xml = dioxus_ssr::render_element(content);
    // TODO: probably it's best to indent the XML as we produce it, but that requires changes to Dioxus SSR.
    // Also we should experiment with how Artilect performs with or w/o indentation.
    if std::env::var("INFER_INDENT_XML").unwrap_or_else(|_| "false".to_string()) == "true" {
        let element = xmltree::Element::parse(xml.as_bytes()).expect("Failed to parse XML");
        let mut output = Vec::new();
        element.write_with_config(&mut output, xmltree::EmitterConfig::new()
            .perform_indent(true)
            .write_document_declaration(false))
            .expect("Failed to write XML");
        String::from_utf8(output).expect("Invalid UTF-8")
    } else {
        xml
    }
}

#[cfg(test)]
mod tests {
    use dioxus_core_macro::component;

    use super::*;
}
