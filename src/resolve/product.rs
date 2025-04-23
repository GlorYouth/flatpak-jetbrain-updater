use crate::error;
use snafu::{whatever, OptionExt, ResultExt};

pub struct ProductInfo {
    paths: Vec<String>,

    short: String,
    name: String,
    lowercase: String,
    code: String,
}

type Pair<'a> = (&'a str, &'a str, &'a str, &'a str);

impl ProductInfo {

    #[inline]
    pub fn new_with_current_dir() -> error::Result<ProductInfo> {
        let mut paths_iter = std::fs::read_dir("./")
            .with_whatever_context(|e| format!("Failed to read directory: {}", e))?
            .into_iter();
        let paths = paths_iter.try_fold(Vec::new(), |mut acc, path| {
            let s = path
                .with_whatever_context(|x| format!("Failed to read directory \"{}\"", x))
                .and_then(|dir| {
                    match dir.file_name().into_string() {
                        Ok(s) => {Ok(s)}
                        Err(e) => {
                            whatever!("Failed to parse file: {:?}", e)
                        }
                    }
                })?;
            acc.push(s);
            Ok(acc)
        })?;
        Self::from_lowcase_name(paths)
            .whatever_context("Failed to find any jetbrains files in current directory")
    }

    fn from_lowcase_name(paths: Vec<String>) -> Option<ProductInfo> {

        // 用静态数组维护关键词与其他参数的对应关系
        const PAIRS: [Pair; 5] = [
            ("clion", "clion", "CLion", "CL"),
            ("rustrover", "rustrover", "RustRover", "RR"),
            ("webstorm", "webstorm", "WebStorm", "WS"),
            ("goland", "goland", "GoLand", "GL"),
            ("pycharm", "pycharm-community", "PyCharm-Community", "PCC"),
        ];

        // 迭代查找：一旦找到包含关键词的，就返回对应
        PAIRS
            .into_iter()
            .find(|(_, lc, _, _)| {
                paths.iter().any(|p| p.to_lowercase().contains(lc))
            })
            .map(|(short, lc, name, code)| ProductInfo {
                paths,

                short: short.to_string(),
                name: name.to_string(),
                lowercase: lc.to_string(),
                code: code.to_string(),
            })
    }
    
    #[inline]
    pub fn find_yaml_from_path(&self) -> Option<String> {
        let possible_paths = [format!("com.jetbrains.{}.yaml", self.name), format!("com.jetbrains.{}.yml", self.name)];
        possible_paths.iter().find_map(|path| {
            self.paths.iter().any(|s| {
                s.eq(path)
            }).then(|| path.to_string())
        })
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn lowercase(&self) -> &str {
        &self.lowercase
    }

    #[inline]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[inline]
    pub fn short(&self) -> &str {
        self.short.as_str()
    }
}
