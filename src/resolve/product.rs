pub struct ProductInfo {
    name: String,
    lowercase: String,
    code: String,
}

impl ProductInfo {
    #[inline]
    pub fn new_with_current_dir() -> Option<ProductInfo> {
        let paths = std::fs::read_dir("./").unwrap();
        for path in paths {
            let file_name = path.unwrap().file_name().into_string().unwrap();
            if let Some(info) = Self::from_name(&file_name) {
                return Some(info);
            }
        }
        None
    }

    fn from_name(name: &str) -> Option<ProductInfo> {
        // 先把输入转为小写，避免多次分配
        let s = name.to_lowercase();

        // 用静态数组维护关键词与其他参数的对应关系
        const PAIRS: [(&str, &str, &str); 5] = [
            ("clion", "CLion", "CL"),
            ("rustrover", "RustRover", "RR"),
            ("webstorm", "WebStorm", "WS"),
            ("goland", "GoLand", "GL"),
            ("pycharm", "PyCharm", "PC"),
        ];

        // 迭代查找：一旦找到包含关键词的，就返回对应
        PAIRS.into_iter().find_map(|(lc, name, code)| {
            if s.contains(lc) {
                Some(ProductInfo {
                    name: name.to_string(),
                    lowercase: lc.to_string(),
                    code: code.to_string(),
                })
            } else {
                None
            }
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
}
