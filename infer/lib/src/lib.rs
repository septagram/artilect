use dioxus::prelude::*;

mod element_constructors;
pub use element_constructors::*;

#[allow(non_snake_case, non_upper_case_globals)]
pub mod dioxus_elements {
    pub use dioxus::html::elements::*; // TODO: remove this
    use super::*;

    builder_constructors! {
        systemPrompt None {
            content: String DEFAULT,
        };
        primeDirectives None {
            content: String DEFAULT,
        };
        directive None {
            content: String DEFAULT,
            isBool: bool DEFAULT,
        };
    }

    pub mod elements {
        pub use super::*;
    }
}

pub use dioxus_elements::*;

#[component]
pub fn SystemPrompt() -> Element {
    rsx! {
        systemPrompt {
            "Line one"
            "Line two"
            "Line three"
            option {
                selected: true,
                hidden: false,
                "Test"
            }
            primeDirectives {
                directive {
                    isBool: false,
                    "Directive one"
                }
                directive {
                    isBool: true,
                    "Directive two"
                }
                directive { "Directive three" }
            }
        }
    }
}

pub fn render_prompt<F>(prompt: F) -> String 
where 
    F: Fn() -> Element,
{
    dioxus_ssr::render_element(rsx!{
        SystemPrompt {}
    })
}

#[cfg(test)]
mod tests {
    use dioxus_core_macro::component;

    use super::*;
}
