#[macro_export]
macro_rules! system_instructions {
    ($($tokens:tt)*) => {
        markup::new!(
            "\n"
            systemInstructions {
                $($tokens)*
            }
        ).to_string().into()
    };
}
