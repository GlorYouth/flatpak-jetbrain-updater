use crate::error::Error;
use crate::utils::{read_json, update_xml, update_yaml};
use serde_json::Value;
use snafu::{OptionExt, ResultExt};

mod error;
mod resolve;
mod utils;

#[tokio::main]
async fn main() -> error::Result<()> {
    let product_info = resolve::ProductInfo::new_with_current_dir()?;
    let json = reqwest::get(format!(
        "https://data.services.jetbrains.com/products/releases?code={}&type=release",
        product_info.code()
    ))
    .await
    .map_err(|e| Error::Network {
        message: e.to_string(),
    })?
    .text()
    .await
    .with_whatever_context(|e| format!("Failed to read response body, source: {:?}", e))?;

    let v: Value = serde_json::from_str(json.as_str()).with_whatever_context(|e| {
        format!(
            "Failed to Failed to parse response body as JSON, source: {:?}",
            e
        )
    })?;
    let array = v[product_info.code()]
        .as_array()
        .with_whatever_context(|| {
            format!(r#"Failed to find "{}" in JSON top"#, product_info.code())
        })?;
    let mut collection = read_json(array)?;

    let xml_path = format!("com.jetbrains.{}.appdata.xml", product_info.name());
    update_xml(xml_path, &mut collection)?;
    update_yaml(&product_info, &mut collection).await?;
    Ok(())
}
