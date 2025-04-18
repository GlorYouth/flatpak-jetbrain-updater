use crate::resolve;
use crate::resolve::{Platform, ProductRelease};
use serde_json::Value;

pub fn read_json(array: &Vec<Value>) -> Vec<ProductRelease> {
    let re = resolve::RE::default();
    array
        .iter()
        .fold(Vec::with_capacity(array.len()), |mut acc, x| {
            fn init_platform(map: &serde_json::Map<String, Value>, is_first: bool) -> Platform {
                let checksum_link = if is_first {
                    Some(resolve::Checksum::from_str(
                        map["checksumLink"].as_str().unwrap(),
                    ))
                } else {
                    None
                };
                Platform {
                    link: map["link"].as_str().unwrap(),
                    size: map["size"].as_u64().unwrap() as usize,
                    checksum_link,
                }
            }
            let download = &x["downloads"];
            if let Some(map) = download["linux"].as_object() {
                let is_first = acc.len() == 0;
                let linux_amd64 = init_platform(map, is_first);
                let linux_arm64 = {
                    if !is_first {
                        None
                    } else if let Some(map) = download["linuxARM64"].as_object() {
                        Some(init_platform(map, is_first))
                    } else {
                        None
                    }
                };
                let release = ProductRelease {
                    date: x.get("date").unwrap().as_str().unwrap(),
                    version: x.get("version").unwrap().as_str().unwrap(),
                    linux_amd64,
                    linux_arm64,
                    re: re.clone(),
                };
                acc.push(release);
            }
            acc
        })
}
