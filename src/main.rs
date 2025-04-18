use crate::utils::{read_json, update_xml, update_yaml};
use serde_json::Value;

mod resolve;
mod utils;

#[tokio::main]
async fn main() {
    let product_info = resolve::ProductInfo::new_with_current_dir().unwrap();
    let json = reqwest::get(format!(
        "https://data.services.jetbrains.com/products/releases?code={}&type=release",
        product_info.code()
    ))
    .await
    .unwrap()
    .text()
    .await
    .unwrap();

    let v: Value = serde_json::from_str(json.as_str()).unwrap();
    let array = v[product_info.code()].as_array().unwrap();
    let mut collection = read_json(array);

    let xml_path = format!("com.jetbrains.{}.appdata.xml", product_info.name());
    update_xml(xml_path, &mut collection);

    let yaml_path = format!("com.jetbrains.{}.yaml", product_info.name());
    update_yaml(yaml_path, &product_info, &mut collection).await;
}
