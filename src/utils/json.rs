use crate::resolve::{Platform, ProductRelease};
use crate::{error, resolve};
use serde_json::Value;
use snafu::OptionExt;

pub fn read_json(array: &Vec<Value>) -> error::Result<Vec<ProductRelease>> {
    let re = resolve::RE::default();
    array
        .iter()
        .try_fold(Vec::with_capacity(array.len()), |mut acc, x| {
            fn init_platform(
                map: &serde_json::Map<String, Value>,
                is_first: bool,
            ) -> error::Result<Platform> {
                let checksum_link = if is_first {
                    Some(resolve::Checksum::from_str(
                        map["checksumLink"].as_str().whatever_context(
                            "Unexpected JSON structure while reading checksumLink",
                        )?,
                    ))
                } else {
                    None
                };
                Ok(Platform {
                    link: map["link"]
                        .as_str()
                        .whatever_context("Unexpected JSON structure while reading link")?,
                    size: map["size"]
                        .as_u64()
                        .whatever_context("Unexpected JSON structure while reading size")?
                        as usize,
                    checksum_link,
                })
            }
            let download = &x["downloads"];
            if let Some(map) = download["linux"].as_object() {
                let is_first = acc.len() == 0;
                let linux_amd64 = init_platform(map, is_first)?;
                let linux_arm64 = {
                    if !is_first {
                        None
                    } else if let Some(map) = download["linuxARM64"].as_object() {
                        Some(init_platform(map, is_first)?)
                    } else {
                        None
                    }
                };
                let release = ProductRelease {
                    date: x
                        .get("date")
                        .whatever_context("Unexpected JSON structure while reading date")?
                        .as_str()
                        .whatever_context("Failed to convert date to string")?,
                    version: x
                        .get("version")
                        .whatever_context("Unexpected JSON structure while reading version")?
                        .as_str()
                        .whatever_context("Failed to convert version to string")?,
                    linux_amd64,
                    linux_arm64,
                    re: re.clone(),
                };
                acc.push(release);
            }
            Ok(acc)
        })
}
