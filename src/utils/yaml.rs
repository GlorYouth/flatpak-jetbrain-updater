use crate::error;
use crate::resolve::{ProductInfo, ProductRelease};
use snafu::{OptionExt, ResultExt, whatever};

use serde_yaml::{Mapping, Value};

trait ValueExt {
    // fn get_map<'a>(&'a self, key: &str, path: &str) -> error::Result<&'a Mapping>;
    // fn get_map_mut<'a>(&'a mut self, key: &str, path: &str) -> error::Result<&'a mut Mapping>;
    // fn get_seq<'a>(&'a self, key: &str, path: &str) -> error::Result<&'a Vec<Value>>;
    fn get_seq_mut<'a>(&'a mut self, key: &str, path: &str) -> error::Result<&'a mut Vec<Value>>;
}

impl ValueExt for Value {
    // #[inline]
    // fn get_map<'a>(&'a self, key: &str, path: &str) -> error::Result<&'a Mapping> {
    //     self.get(key)
    //         .with_whatever_context(|| format!("Missing '{}' in YAML at {}", key, path))?
    //         .as_mapping()
    //         .with_whatever_context(|| format!("'{}' is not a mapping at {}", key, path))
    // }
    //
    // #[inline]
    // fn get_map_mut<'a>(&'a mut self, key: &str, path: &str) -> error::Result<&'a mut Mapping> {
    //     self.get_mut(key)
    //         .with_whatever_context(|| format!("Missing '{}' in YAML at {}", key, path))?
    //         .as_mapping_mut()
    //         .with_whatever_context(|| format!("'{}' is not a mapping at {}", key, path))
    // }
    // #[inline]
    // fn get_seq<'a>(&'a self, key: &str, path: &str) -> error::Result<&'a Vec<Value>> {
    //     self.get(key)
    //         .with_whatever_context(|| format!("Missing '{}' in YAML at {}", key, path))?
    //         .as_sequence()
    //         .with_whatever_context(|| format!("'{}' is not a sequence at {}", key, path))
    // }

    #[inline]
    fn get_seq_mut<'a>(&'a mut self, key: &str, path: &str) -> error::Result<&'a mut Vec<Value>> {
        self.get_mut(key)
            .with_whatever_context(|| format!("Missing '{}' in YAML at {}", key, path))?
            .as_sequence_mut()
            .with_whatever_context(|| format!("'{}' is not a sequence at {}", key, path))
    }
}

trait MappingEx {
    fn get_mut_err<'a>(&'a mut self, key: &str, path: &str) -> error::Result<&'a mut Value>;
}

impl MappingEx for Mapping {
    #[inline]
    fn get_mut_err<'a>(&'a mut self, key: &str, path: &str) -> error::Result<&'a mut Value> {
        Mapping::get_mut(self, key)
            .with_whatever_context(|| format!("Failed to find {} in yaml, path: {}", key, path))
    }
}

#[inline]
fn read_yaml(path: &str) -> error::Result<String> {
    std::fs::read_to_string(&path)
        .with_whatever_context(|e| format!("Failed to read yaml file at {}, source: {:?}", path, e))
}

#[inline]
fn parse_yaml(yaml: String, yaml_path: &str) -> error::Result<Value> {
    serde_yaml::from_str::<Value>(yaml.as_str()).with_whatever_context(|e| {
        format!(
            "Failed to parse yaml file at {}, source: {:?}",
            yaml_path, e
        )
    })
}

fn find_named_map<'a>(
    modules: &'a mut Vec<Value>,
    product_info: &ProductInfo,
    yaml_path: &str,
) -> error::Result<&'a mut Mapping> {
    modules
        .iter_mut()
        .find_map(|x| {
            x.as_mapping_mut().and_then(|mapping| {
                mapping
                    .get_mut_err("name", yaml_path)
                    .and_then(|v| {
                        v.as_str()
                            .with_whatever_context(|| {
                                format!("Failed to convert name in YAML, path: {}", yaml_path)
                            })
                            .map(|name| name == product_info.short() || name == product_info.name())
                    })
                    .map(|matched| matched.then_some(mapping))
                    .transpose()
            })
        })
        .with_whatever_context(|| {
            format!(
                "Failed to find {} in YAML, path: {}",
                product_info.short(),
                yaml_path
            )
        })?
}

struct Platforms<'a> {
    x86_64: &'a mut Mapping,
    aarch64: Option<&'a mut Mapping>,
}

impl<'a> Platforms<'a> {
    fn from_collected(
        collected: &'a mut Vec<&'a mut Mapping>,
        product_info: &ProductInfo,
        yaml_path: &str,
    ) -> error::Result<Platforms<'a>> {
        if collected.is_empty() {
            whatever!(
                "Cannot find any '{}.tar.gz' file in YAML, path: {}",
                product_info.lowercase(),
                yaml_path
            );
        };
        if collected.len() == 1 {
            Ok(Platforms {
                x86_64: &mut collected[0],
                aarch64: None,
            })
        } else {
            let mut units = [("x86_64", None), ("aarch64", None)];
            for i in 0..collected.len() {
                collected[i].get_mut_err("only-arches", yaml_path).and_then(|v| {
                    let seq = v.as_sequence().with_whatever_context(|| {
                        format!("Unexpected YAML structure while reading only-arches, path: {}", yaml_path)
                    })?;
                    let seq_str = seq.first().with_whatever_context(|| {
                        format!("The only-arches sequence contain no values in YAML, path: {}", yaml_path)
                    })?.as_str().with_whatever_context(|| {
                        format!("Failed to convert only-arches first element to string in YAML, path: {}",yaml_path)
                    })?;
                    for i in 0..units.len() {
                        if units[i].0.eq(seq_str) {
                            if units[i].1.is_some() {
                                whatever!("There are conflict arch software in YAML, path: {}", yaml_path);
                            }
                            units[i].1 = Some(i);
                        }
                    }
                    Ok(())
                })?;
            }
            if let Some(x86_64_pos) = units[0].1 {
                return if let Some(aarch64_pos) = units[1].1 {
                    let [x86_64, aarch64] = collected
                        .get_disjoint_mut([x86_64_pos, aarch64_pos])
                        .with_whatever_context(|e| {
                        format!(
                            "The pos of different arches is conflict, this is a bug, source: {}",
                            e
                        )
                    })?;
                    Ok(Platforms {
                        x86_64,
                        aarch64: Some(aarch64),
                    })
                } else {
                    let x86_64 = &mut collected[x86_64_pos];
                    Ok(Platforms {
                        x86_64,
                        aarch64: None,
                    })
                };
            }
            whatever!("Failed to find x86_64 in YAML path: {}", yaml_path)
        }
    }

    fn write_from_release(
        &mut self,
        product_release: &ProductRelease,
        yaml_path: &str,
    ) -> error::Result<()> {
        use crate::resolve::Platform;
        let write = |map: &mut Mapping, platform: &Platform| -> error::Result<()> {
            if map.contains_key("size") {
                *map.get_mut_err("size", yaml_path)? =
                    Value::Number(serde_yaml::Number::from(platform.size));
            }
            *map.get_mut_err("url", yaml_path)? = Value::String(platform.link.to_string());
            let checksum = platform
                .checksum_link
                .as_ref()
                .whatever_context("Checksum has not been requested from the server, this is a bug")?
                .clone();
            let (_type, _res) = checksum.into_type_and_res();
            if !_type.eq("sha256") {
                whatever!("Different checksum type");
            }
            *map.get_mut_err("sha256", yaml_path)? = Value::String(_res.clone());
            Ok(())
        };

        write(&mut self.x86_64, &product_release.linux_amd64)?;

        if let Some(aarch64) = &mut self.aarch64 {
            write(
                aarch64,
                product_release
                    .linux_arm64
                    .as_ref()
                    .whatever_context("Failed to find latest aarch64 in JSON")?,
            )?;
        }
        Ok(())
    }
}

fn collect_platforms<'a>(
    named_map: &'a mut Mapping,
    product_info: &ProductInfo,
    yaml_path: &str,
) -> error::Result<Vec<&'a mut Mapping>> {
    let vec = named_map
        .get_mut_err("sources", yaml_path)?
        .as_sequence_mut()
        .with_whatever_context(|| {
            format!(
                "Unexpected YAML structure while reading sources, path: {}",
                yaml_path
            )
        })?
        .iter_mut()
        .filter(|v| {
            static KEYS: &[&str] = &["filename", "dest-filename"];
            v.is_mapping()
                && KEYS.iter().any(|key| {
                    v.as_mapping().unwrap().contains_key(key)
                        && v.as_mapping().unwrap()[key]
                            .eq(&format!("{}.tar.gz", product_info.lowercase()))
                })
        })
        .collect::<Vec<&mut Value>>();
    let maps = Vec::with_capacity(vec.len());
    vec.into_iter().try_fold(maps, |mut vec, v| {
        let map = v.as_mapping_mut().with_whatever_context(|| {
            format!(
                "Unexpected YAML structure while collect platforms, path: {}",
                yaml_path
            )
        })?;
        vec.push(map);
        Ok(vec)
    })
}

pub async fn update_yaml(
    product_info: &ProductInfo,
    collection: &mut Vec<ProductRelease<'_>>,
) -> error::Result<()> {
    let yaml_path = product_info.find_yaml_from_path().whatever_context("Failed to find YAML path")?;
    
    let yaml = read_yaml(&yaml_path)?;
    let mut root = parse_yaml(yaml, &yaml_path)?;
    let modules = root.get_seq_mut("modules", &yaml_path)?;
    let named_map = find_named_map(modules, product_info, &yaml_path)?;
    let mut collected = collect_platforms(named_map, &product_info, &yaml_path)?;
    let mut platforms = Platforms::from_collected(&mut collected, &product_info, &yaml_path)?;

    if collection.len() == 0 {
        println!("It is up to date");
        return Ok(());
    }

    let client = reqwest::Client::new();
    collection[0].complete_checksum(client).await;
    platforms.write_from_release(&collection[0], &yaml_path)?;

    let yaml_str =
        serde_yaml::to_string(&root).whatever_context("Failed to serialize YAML, this is a bug")?;
    std::fs::write(&yaml_path, yaml_str).with_whatever_context(|e| {
        format!("Failed to write YAML to {}, source: {:?}", yaml_path, e)
    })?;

    Ok(())
}
