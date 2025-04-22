mod json;
mod xml;
mod yaml;

pub use json::read_json;
pub use xml::update_xml;
pub use yaml::update_yaml;

#[macro_export]
macro_rules! get_with_check {
    ($item:ident,$target:ident,$supplement:literal) => {
        $item.get(stringify!($target)).with_whatever_context(|| {
            format!(
                "Failed to find {} {}",
                stringify!($target),
                $supplement
            )
        })?
    };
}
