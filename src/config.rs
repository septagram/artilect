#[cfg(any(feature = "backend", feature = "infer"))]
pub mod back_shared;

pub fn validate() {
    #[cfg(any(feature = "backend", feature = "infer"))]
    back_shared::validate();
    
    #[cfg(feature = "infer")]
    crate::infer::config::validate();
}
