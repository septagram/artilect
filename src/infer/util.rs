use serde::Serialize;
use textwrap::{wrap, Options};
use regex::Regex;

pub fn wrap_and_indent_yaml<T: Serialize + std::fmt::Debug>(value: &T) -> String {
    let yaml = serde_yaml::to_string(value).unwrap_or_else(|_| format!("{:?}", value));
    let indent_re = Regex::new(r"^(\s*)").unwrap();
    yaml.lines()
        .map(|line| {
            let indent = indent_re.captures(line)
                .and_then(|caps| caps.get(1))
                .map(|m| m.as_str())
                .unwrap_or("");
            let content = line.trim_start();
            
            if content.is_empty() {
                line.to_string()
            } else {
                wrap(content, Options::new(80)
                    .break_words(false)
                    .word_separator(textwrap::WordSeparator::AsciiSpace)
                    .word_splitter(textwrap::WordSplitter::NoHyphenation)
                    .initial_indent(indent)
                    // .subsequent_indent(&format!("{}  ", indent)))
                    .subsequent_indent(indent))
                    .join("\n")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
