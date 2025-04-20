use crate::error;
use crate::resolve::{ProductInfo, ProductRelease};
use snafu::{whatever, OptionExt, ResultExt};

pub async fn update_yaml(
    yaml_path: String,
    product_info: &ProductInfo,
    collection: &mut Vec<ProductRelease<'_>>,
) -> error::Result<()> {
    let yaml = std::fs::read_to_string(&yaml_path).with_whatever_context(|e| {
        format!("Failed to read yaml file at {}, source: {:?}", yaml_path, e)
    })?;
    let mut v =
        serde_yaml::from_str::<serde_yaml::Value>(yaml.as_str()).with_whatever_context(|e| {
            format!(
                "Failed to parse yaml file at {}, source: {:?}",
                yaml_path, e
            )
        })?;
    let platforms = &mut v["modules"]
        .as_sequence_mut()
        .with_whatever_context(|| {
            format!(
                "Unexpected YAML structure while reading modules, path: {}",
                yaml_path
            )
        })?
        .iter_mut()
        .find_map(|x| {
            x.as_mapping_mut().and_then(|mapping| {
                mapping["name"]
                    .as_str()
                    .with_whatever_context(|| {
                        format!("Failed to convert name in YAML, path: {}", yaml_path)
                    })
                    .map(|name| name == product_info.lowercase())
                    .map(|matched| matched.then_some(mapping))
                    .transpose()
            })
        })
        .with_whatever_context(|| {
            format!(
                "Failed to find {} in YAML, path: {}",
                product_info.lowercase(),
                yaml_path
            )
        })??["sources"]
        .as_sequence_mut()
        .with_whatever_context(|| {
            format!(
                "Unexpected YAML structure while reading sources, path: {}",
                yaml_path
            )
        })?
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
        .find_map(|v| {
            v["only-arches"]
                .as_sequence_mut()
                .with_whatever_context(|| {
                    format!(
                        "Unexpected YAML structure while reading only-arches, path: {}",
                        yaml_path
                    )
                })
                .map(|seq| seq[0].eq("x86_64").then_some(&mut seq[0]))
                .transpose()
        })
        .with_whatever_context(|| {
            format!("Failed to find x86_64 in YAML, path: {}unwrap()", yaml_path)
        })??;
    if collection.len() == 0 {
        println!("It is up to date");
        return Ok(());
    }
    let client = reqwest::Client::new();
    collection[0].complete_checksum(client).await;
    let json_amd64 = &collection[0].linux_amd64;
    x86_64["size"] =
        serde_yaml::Value::Number(serde_yaml::Number::from(json_amd64.size));
    x86_64["url"] = serde_yaml::Value::String(json_amd64.link.to_string());
    let checksum = json_amd64
        .checksum_link
        .as_ref()
        .whatever_context("Checksum has not been requested from the server, this is a bug")?
        .clone();
    let (_type, _res) = checksum.into_type_and_res();
    if !_type.eq("sha256") {
        whatever!("Different checksum type");
    }
    x86_64["sha256"] = serde_yaml::Value::String(_res.clone());

    if let Some(aarch64) = platforms
        .iter_mut()
        .find_map(|v| {
            v["only-arches"]
                .as_sequence_mut()
                .with_whatever_context(|| {
                    format!(
                        "Unexpected YAML structure while reading only-arches, path: {}",
                        yaml_path
                    )
                })
                .map(|seq| seq[0].eq("aarch64").then_some(&mut seq[0]))
                .transpose()
        })
    {
        let aarch64 = aarch64?;
        let json_aarch64 = collection[0].linux_arm64.as_ref().whatever_context("Failed to find latest aarch64 in JSON")?;
        aarch64["size"] = serde_yaml::Value::Number(serde_yaml::Number::from(
            json_aarch64.size,
        ));
        aarch64["url"] =
            serde_yaml::Value::String(json_aarch64.link.to_string());
        let checksum = json_aarch64
            .checksum_link
            .as_ref()
            .whatever_context("Checksum has not been requested from the server, this is a bug")?
            .clone();
        let (_type, _res) = checksum.into_type_and_res();
        if !_type.eq("sha256") {
            whatever!("Different checksum type");
        }
        aarch64["sha256"] = serde_yaml::Value::String(_res.clone());
    }
    let yaml_str = serde_yaml::to_string(&v).whatever_context("Failed to serialize YAML, this is a bug")?;
    std::fs::write(&yaml_path, yaml_str).with_whatever_context(|e| format!("Failed to write YAML to {}, source: {:?}", yaml_path, e))?;
    Ok(())
}
