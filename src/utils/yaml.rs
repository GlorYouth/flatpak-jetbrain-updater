use crate::resolve::{ProductInfo, ProductRelease};

pub async fn update_yaml(
    yaml_path: String,
    product_info: &ProductInfo,
    collection: &mut Vec<ProductRelease<'_>>,
) {
    let yaml = std::fs::read_to_string(&yaml_path).unwrap();
    let mut v = serde_yaml::from_str::<serde_yaml::Value>(yaml.as_str()).unwrap();
    let platforms = &mut v["modules"]
        .as_sequence_mut()
        .unwrap()
        .iter_mut()
        .find(|x| {
            x.is_mapping()
                && x.as_mapping().unwrap()["name"]
                    .as_str()
                    .unwrap()
                    .eq(product_info.lowercase())
        })
        .unwrap()["sources"]
        .as_sequence_mut()
        .unwrap()
        .iter_mut()
        .filter(|v| {
            v.is_mapping()
                && v.as_mapping().unwrap().contains_key("filename")
                && v.as_mapping().unwrap()["filename"]
                    .eq(&format!("{}.tar.gz", product_info.lowercase()))
        })
        .collect::<Vec<&mut serde_yaml::Value>>();

    let x86_64 = platforms
        .iter_mut()
        .find(|v| v["only-arches"].as_sequence().unwrap()[0].eq("x86_64"))
        .unwrap()
        .as_mapping_mut()
        .unwrap();
    if collection.len() == 0 {
        println!("It is up to date");
        return;
    }
    x86_64["size"] =
        serde_yaml::Value::Number(serde_yaml::Number::from(collection[0].linux_amd64.size));
    let client = reqwest::Client::new();
    collection[0].complete_checksum(client).await;
    x86_64["url"] = serde_yaml::Value::String(collection[0].linux_amd64.link.to_string());
    let checksum = collection[0]
        .linux_amd64
        .checksum_link
        .as_ref()
        .unwrap()
        .clone();
    let (_type, _res) = checksum.into_type_and_res();
    if !_type.eq("sha256") {
        panic!("Different checksum type");
    }
    x86_64["sha256"] = serde_yaml::Value::String(_res.clone());

    print!("{:?}",collection[0]);

    if let Some(aarch64) = platforms
        .iter_mut()
        .find(|v| v["only-arches"].as_sequence().unwrap()[0].eq("aarch64"))
    {
        let aarch64 = aarch64.as_mapping_mut().unwrap();
        aarch64["size"] = serde_yaml::Value::Number(serde_yaml::Number::from(
            collection[0].linux_arm64.as_ref().unwrap().size,
        ));
        aarch64["url"] =
            serde_yaml::Value::String(collection[0].linux_arm64.as_ref().unwrap().link.to_string());
        let checksum = collection[0]
            .linux_arm64
            .as_ref()
            .unwrap()
            .checksum_link
            .as_ref()
            .unwrap()
            .clone();
        let (_type, _res) = checksum.into_type_and_res();
        if !_type.eq("sha256") {
            panic!("Different checksum type");
        }
        aarch64["sha256"] = serde_yaml::Value::String(_res.clone());
    }
    let yaml_str = serde_yaml::to_string(&v).unwrap();
    std::fs::write(yaml_path, yaml_str).unwrap();
}
