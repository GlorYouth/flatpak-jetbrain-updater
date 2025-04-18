use regex::Regex;
use reqwest::Client;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Checksum<'a> {
    Link(&'a str),
    TypeAndRes(String, String),
}

impl<'a> Checksum<'a> {
    #[inline]
    pub fn from_str(s: &str) -> Checksum {
        Checksum::Link(s)
    }

    #[inline]
    pub fn into_type_and_res(self) -> (String, String) {
        if let Checksum::TypeAndRes(s1, s2) = self {
            (s1, s2)
        } else {
            unreachable!()
        }
    }
}

#[derive(Debug)]
pub struct Platform<'a> {
    pub link: &'a str,
    pub size: usize,
    pub checksum_link: Option<Checksum<'a>>,
}

#[derive(Clone, Debug)]
pub struct RE {
    re: Arc<Regex>,
}

impl Default for RE {
    fn default() -> RE {
        RE {
            re: Arc::new(Regex::new("^[A-Za-z0-9]+").unwrap()),
        }
    }
}

#[derive(Debug)]
pub struct ProductRelease<'a> {
    pub date: &'a str,
    pub version: &'a str,
    pub linux_amd64: Platform<'a>,
    pub linux_arm64: Option<Platform<'a>>,
    pub re: RE,
}

impl<'a> ProductRelease<'a> {
    pub async fn complete_checksum(&mut self, client: Client) {
        let get_checksum = async |url: &str| -> Option<String> {
            for i in 0..3 {
                if i > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(i)).await;
                }
                let result = client.get(url).send().await;
                match result {
                    Ok(res) => {
                        let text = res
                            .text()
                            .await
                            .ok()
                            .map(|t| self.re.re.find(t.as_str()).unwrap().as_str().to_owned());
                        return text;
                    }
                    Err(e) => {
                        println!("Failed to get release checksum: {}", e);
                    }
                }
            }
            None
        };

        if let Some(Checksum::Link(link)) = &self.linux_amd64.checksum_link {
            let tp = link.rsplit('.').next().unwrap().to_string();
            self.linux_amd64.checksum_link =
                Some(Checksum::TypeAndRes(tp, get_checksum(link).await.unwrap()));
        }
        if let Some(checksum) = &mut self.linux_arm64 {
            if let Some(Checksum::Link(link)) = checksum.checksum_link {
                let tp = link.rsplit('.').next().unwrap().to_string();
                checksum.checksum_link =
                    Some(Checksum::TypeAndRes(tp, get_checksum(link).await.unwrap()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_complete_checksum() {
        let client = Client::default();
        let checksum =
            Checksum::Link("https://download.jetbrains.com/webstorm/WebStorm-2025.1.tar.gz.sha256");
        let size = 0;
        let link = Default::default();
        let mut relase = ProductRelease {
            date: "",
            version: "",
            linux_amd64: Platform {
                link,
                size,
                checksum_link: Some(checksum),
            },
            linux_arm64: None,
            re: RE::default(),
        };
        relase.complete_checksum(client).await;
        println!("release: {:?}", relase);
    }
}
