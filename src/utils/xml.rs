// 引入项目内错误处理模块
use crate::error;
// 引入项目内 ProductRelease 结构体定义
use crate::resolve::ProductRelease;
// 引入项目内 XML 工具库中的错误消息生成函数
use crate::utils::xml::_err::failed_to_write_event;

// 从 quick_xml 库引入处理 XML 属性所需的相关类型
use quick_xml::events::attributes::{Attribute, Attributes};
// 从 quick_xml 库引入 XML 事件类型，如开始标签、结束标签等
use quick_xml::events::{BytesEnd, Event};
// 从 quick_xml 库引入 XML 读取器和写入器
use quick_xml::{Reader, Writer};

// 从 snafu 库引入用于错误处理的扩展 Trait 和宏
use snafu::{OptionExt, ResultExt, whatever}; // whatever! 用于创建临时的、上下文相关的错误

// 从标准库引入 Cow (Clone-on-Write) 智能指针，用于处理可能借用或拥有的字符串数据
use std::borrow::Cow;
// 从标准库引入 Cursor，用于在内存中的字节向量上实现 Read 和 Write Trait
use std::io::Cursor;

// 定义一个内部模块，用于存放错误处理相关的辅助函数
mod _err {
    /// 生成一个表示写入 XML 事件失败的错误消息字符串。
    ///
    /// # Arguments
    ///
    /// * `e` - 一个指向 `std::io::Error` 的可变引用。
    ///
    /// # Returns
    ///
    /// * `String` - 格式化后的错误消息。
    #[inline] // 建议编译器内联此函数，减少函数调用开销
    pub fn failed_to_write_event(e: &mut std::io::Error) -> String {
        format!("Failed to write event to XML: {:?}", e)
    }
}

/// 定义一个 Trait，用于将实现了该 Trait 的类型写入到 XML Writer 中。
trait XMLWriter {
    /// 将自身的 XML 表示写入到提供的 Writer 中。
    ///
    /// # Arguments
    ///
    /// * `writer` - 一个 `quick_xml::Writer` 的可变引用，写入目标是内存中的 `Vec<u8>`。
    ///
    /// # Returns
    ///
    /// * `error::Result<()>` - 如果写入成功，返回 `Ok(())`，否则返回项目定义的错误类型。
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> error::Result<()>;

    /// 返回与此 XML 片段关联的日期字符串引用。
    /// 用于后续的排序。
    fn date(&self) -> &str;
}

/// 为 `ProductRelease` 结构体实现 `XMLWriter` Trait。
/// 这允许我们将一个新的产品发布信息格式化为 XML。
impl XMLWriter for ProductRelease<'_> {
    /// 将 `ProductRelease` 写入为一个空的 `<release>` XML 标签。
    /// 例如：`<release version="1.0.0" date="2023-01-01"/>`
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> error::Result<()> {
        writer
            .create_element("release") // 创建名为 "release" 的元素
            .with_attribute(Attribute::from(("version", self.version))) // 添加 "version" 属性
            .with_attribute(Attribute::from(("date", self.date))) // 添加 "date" 属性
            .write_empty() // 写入为一个空标签（自闭合标签）
            .with_whatever_context(failed_to_write_event)?; // 如果写入失败，使用 `failed_to_write_event` 生成错误信息
        Ok(())
    }

    /// 返回 `ProductRelease` 中的日期字符串引用。
    #[inline]
    fn date(&self) -> &str {
        &self.date
    }
}

/// 为元组 `(String, Vec<Event<'_>>)` 实现 `XMLWriter` Trait。
/// 这个元组用于表示从原始 XML 文件中保留下来的、需要原样写回的 XML 片段（通常是一个完整的 <release>...</release> 块）。
/// 元组的第一个元素 `String` 存储该片段的日期，用于排序。
/// 元组的第二个元素 `Vec<Event<'_>>` 存储构成该 XML 片段的 `quick_xml` 事件序列。
impl XMLWriter for (String, Vec<Event<'_>>) {
    /// 将存储的 XML 事件序列依次写入到 Writer 中。
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> error::Result<()> {
        // 遍历存储的事件列表
        for event in self.1.iter() {
            // 注意：这里克隆了 event。Event<'a> 可能包含借用数据，克隆会创建拥有的副本。
            // 这是必要的，因为我们需要在读取完成后，将这些事件写入新的 Writer。
            writer
                .write_event(event.clone()) // 将事件写入 Writer
                .with_whatever_context(failed_to_write_event)?; // 处理写入错误
        }
        Ok(())
    }

    /// 返回元组中存储的日期字符串的引用。
    #[inline]
    fn date(&self) -> &str {
        self.0.as_str()
    }
}

/// XML 处理程序结构体，封装了处理 XML 更新逻辑所需的状态。
/// 生命周期参数说明：
/// 'a: XML 输入数据的生命周期（来自 xml_bytes）。
/// 'b: ProductRelease 中字符串数据的生命周期（通常是 'static 或与输入 Vec 相关）。
/// 'c: 对外部传入的 Vec<ProductRelease> 的可变引用的生命周期。
struct XMLHandler<'a, 'b, 'c> {
    path: &'a str, // 正在处理的 XML 文件路径，主要用于错误报告
    writer: Writer<Cursor<Vec<u8>>>, // 用于构建新 XML 内容的写入器
    reader: Reader<&'a [u8]>, // 用于读取原始 XML 内容的读取器
    vec: &'c mut Vec<ProductRelease<'b>>, // 外部传入的、包含新发布信息的可变 Vec 的引用
    preserved_xml_fragments: Vec<(String, Vec<Event<'a>>)>, // 用于存储需要从原 XML 保留下来的发布片段（日期和对应的事件序列）
}

/// 宏，用于简化处理 XML 读取事件时产生的错误。
/// 它会创建一个包含错误位置、文件路径和原始错误源的 `Whatever` 错误。
macro_rules! handle_read_event_err {
    // $s: XMLHandler 实例的标识符
    // $e: 发生的错误的标识符
    ($s:ident,$e:ident) => {
        whatever!(
            // 构造错误消息字符串
            r#"Error happen at position {} while parsing XML in "{}", source: {:?}"#,
            // 获取读取器报告的错误位置
            $s.reader.error_position(),
            // 获取文件路径
            $s.path,
            // 包含原始错误信息
            $e
        )
    };
}

impl<'a, 'b, 'c> XMLHandler<'a, 'b, 'c> {
    /// 处理 `</releases>` 结束标签的逻辑。
    /// 在这里，合并新的发布信息和保留的旧发布信息，并按日期排序后写入 Writer。
    ///
    /// # Arguments
    ///
    /// * `e` - `</releases>` 标签对应的 `BytesEnd` 事件。
    ///
    /// # Returns
    ///
    /// * `error::Result<()>` - 成功或失败。
    fn handle_end_of_releases(&mut self, e: BytesEnd) -> error::Result<()> {
        // 注意：写入 BOM (Byte Order Mark) 的代码被注释掉了。
        // 如果需要确保输出文件有 UTF-8 BOM，可以取消注释下面这几行。
        // self.writer
        //     .write_bom()
        //     .with_whatever_context(failed_to_write_event)?;

        // 从 `self.vec` (新的发布信息) 中移除那些日期已经存在于 `preserved_xml_fragments` (保留的旧发布片段) 中的条目。
        // 这意味着如果新旧发布信息有相同的日期，旧的会被保留，新的会被丢弃。
        // retain 方法会保留那些闭包返回 true 的元素。
        self.vec
            .retain(|new_release| !self.preserved_xml_fragments.iter().any(|(preserved_date, _)| new_release.date.eq(preserved_date)));


        // 创建一个新的 Vec，用于存放所有需要写入的发布信息（包括处理过的 `self.vec` 和 `preserved_xml_fragments`）。
        // Vec 的元素类型是 `&dyn XMLWriter`，这是一个 trait object，允许我们混合存储不同类型但都实现了 `XMLWriter` trait 的引用。
        let mut vec_new: Vec<&dyn XMLWriter> = Vec::from_iter(
            self.vec // 迭代处理后剩下的新发布信息
                .iter()
                .map(|x| x as &dyn XMLWriter) // 将 &ProductRelease 转换为 &dyn XMLWriter
                .chain(self.preserved_xml_fragments.iter().map(|e| e as &dyn XMLWriter)), // 链接上保留的旧片段的迭代器，并将 &(String, Vec<Event>) 转换为 &dyn XMLWriter
        );

        // 使用不稳定排序（可能更快，但不保证相等元素的相对顺序）按日期对合并后的列表进行排序。
        // `sort_unstable_by_key` 使用 `XMLWriter` trait 的 `date()` 方法获取排序的键（日期字符串）。
        vec_new.sort_unstable_by_key(|e| e.date());

        // 逆序遍历排序后的列表，并将每个元素写入到 XML Writer 中。
        // 逆序写入可能是为了让最新的发布信息出现在 XML 文件的最前面（这取决于日期的格式和排序）。
        for xw in vec_new.iter().rev() {
            xw.write_xml(&mut self.writer)?; // 调用每个元素的 write_xml 方法
        }

        // 最后，写入 `</releases>` 结束标签。
        self.writer
            .write_event(Event::End(e)) // 写入结束事件
            .with_whatever_context(failed_to_write_event) // 处理写入错误
    }

    /// 处理 `<releases>` 标签内部的事件。
    /// 主要逻辑是识别 `<release>` 标签，判断是否需要保留，并存储需要保留的片段。
    ///
    /// # Returns
    ///
    /// * `error::Result<()>` - 成功或失败。
    fn handle_releases(&mut self) -> error::Result<()> {
        /// 辅助函数：在 `<release>` 标签的属性中搜索 "date" 属性。
        /// 并检查这个日期是否存在于传入的 `vec` (新的发布信息列表) 中。
        ///
        /// # Arguments
        ///
        /// * `attr` - 从 `<release>` 标签读取到的属性迭代器。
        /// * `vec` - 新的 `ProductRelease` 列表的引用，用于检查日期是否存在。
        ///
        /// # Returns
        ///
        /// * `error::Result<(bool, Cow<'a, [u8]>)>` - 返回一个元组：
        ///     - `bool`: 表示该日期是否存在于 `vec` 中。
        ///     - `Cow<'a, [u8]>`: 日期属性的值 (可能是借用的，也可能是拥有的)。
        fn search_date<'a>(
            mut attr: Attributes<'a>, // 接收属性迭代器
            vec: &Vec<ProductRelease>, // 接收新发布信息的引用
        ) -> error::Result<(bool, Cow<'a, [u8]>)> {
            // 查找名为 "date" 的属性
            let value = attr
                .find_map(|x| { // 遍历属性
                    x.with_whatever_context(|e| { // 处理可能的属性解析错误
                        format!("Failed to parse release tag's attribute, source: {:?}", e)
                    })
                        .map(|attr| attr.key.0.eq(b"date").then_some(attr.value)) // 如果键是 "date"，则返回 Some(attr.value)
                        .transpose() // 将 Result<Option<T>, E> 转换为 Option<Result<T, E>>
                })
                .whatever_context("Failed to find date attribute in release tag")??; // 如果未找到或解析出错，返回错误

            // 检查找到的日期值是否存在于 `vec` 中
            Ok((
                // any() 检查迭代器中是否有任何元素满足条件
                vec.iter().any(|r| r.date.as_bytes().eq(value.as_ref())), // 将日期字符串转为字节进行比较
                value, // 返回找到的日期值
            ))
        }

        // 标志位，指示当前是否正在处理一个应该被跳过（即不保留）的 <release>...</release> 块
        let mut is_skip_release = false;

        // 循环处理 <releases> 标签内部的事件
        loop {
            match self.reader.read_event() {
                // 匹配到 </releases> 结束标签
                Ok(Event::End(e)) if e.name().as_ref() == b"releases" => {
                    // 调用处理结束标签的函数，并返回其结果
                    return self.handle_end_of_releases(e);
                }
                // 匹配到文件结束符 (EOF)
                Ok(Event::Eof) => {
                    // 在 <releases> 标签内部遇到 EOF 是意外情况，报告错误
                    whatever!(
                        r#"Unexpected EOF found in releases tag while parsing XML in XMLWriter "{}""#,
                        self.path
                    )
                }
                // 匹配到 <release ...> 开始标签
                Ok(Event::Start(e)) if e.name().as_ref() == b"release" => {
                    is_skip_release = false; // 重置跳过标志
                    // 搜索日期属性，并检查是否存在于新发布列表中
                    let (is_exist, value) = search_date(e.attributes(), self.vec)?;
                    if is_exist {
                        // 如果日期存在于新列表中 (意味着这个旧版本要保留)
                        self.preserved_xml_fragments.push((
                            // 将日期值 (字节) 转换为 UTF-8 字符串，并存储
                            String::from_utf8(value.as_ref().to_vec())
                                .whatever_context("Failed to convert string to UTF-8 after search_date, this is a bug, please report it and post logs.")?,
                            // 将当前 <release> 开始标签事件存入新片段的事件列表中
                            vec![Event::Start(e.to_owned())], // e.to_owned() 创建事件的拥有副本
                        ));
                    } else {
                        // 如果日期不存在于新列表中 (意味着这个旧版本要被删除)
                        is_skip_release = true; // 设置跳过标志
                    }
                }
                // 匹配到 <release ... /> 空标签（自闭合标签）
                Ok(Event::Empty(e)) if e.name().as_ref() == b"release" => {
                    // 逻辑与 Event::Start 类似
                    let (is_exist, value) = search_date(e.attributes(), self.vec)?;
                    if is_exist {
                        // 如果日期存在，保留这个空标签事件
                        self.preserved_xml_fragments.push((
                            String::from_utf8(value.as_ref().to_vec())
                                .whatever_context("Failed to convert string to UTF-8 after search_date, this is a bug, please report it and post logs.")?,
                            vec![Event::Empty(e.to_owned())], // 存储空标签事件的拥有副本
                        ));
                    } else {
                        // 如果日期不存在，标记为跳过（虽然空标签没有后续内容，但保持逻辑一致性）
                        is_skip_release = true;
                    }
                }
                // 匹配到 </release> 结束标签
                Ok(Event::End(e)) if e.name().as_ref() == b"release" => {
                    if !is_skip_release {
                        // 如果没有设置跳过标志 (即这个 release 块是被保留的)
                        // 获取最后添加的那个保留片段 (应该是刚刚处理的 <release> 对应的片段)
                        self.preserved_xml_fragments
                            .last_mut() // 获取最后一个元素的可变引用
                            .whatever_context("Failed to find last mut in preserved_xml_fragments, this is a bug, please report it and post logs.")? // 处理获取失败的理论上不可能发生的错误
                            .1 // 访问片段中的事件 Vec
                            .push(Event::End(e.to_owned())); // 将 </release> 结束标签事件添加到该片段的事件列表中
                    } else {
                        // 如果设置了跳过标志，我们就不保存这个结束标签
                        // 重置跳过标志，为下一个可能的 <release> 块做准备
                        is_skip_release = false;
                    }
                }
                // 匹配到其他任何 XML 事件 (如文本、注释、CDATA 等)
                Ok(e) => {
                    if !is_skip_release {
                        // 如果当前没有在跳过 release 块
                        // 将这个事件添加到最后一个正在记录的保留片段中
                        // 这确保了 <release> 和 </release> 之间的所有内容都被捕获
                        if let Some(last_fragment) = self.preserved_xml_fragments.last_mut() {
                            last_fragment.1.push(e.to_owned()); // 需要克隆事件以获得所有权
                        }
                        // 注意：如果 `preserved_xml_fragments` 为空（例如在第一个 <release> 之前有其他内容），
                        // 并且这些内容不在 `handle_releases` 范围之外处理，那么这些事件可能会丢失。
                        // 不过，从 `start` 函数的逻辑来看，在 `<releases>` 标签之外的事件会被直接写入 Writer。
                    }
                    // 如果 `is_skip_release` 为 true，则忽略此事件
                }
                // 处理读取事件时发生的错误
                Err(e) => {
                    // 使用预定义的宏来创建并返回错误
                    return handle_read_event_err!(self, e);
                }
            }
        }
    }

    /// 开始处理整个 XML 文档。
    /// 循环读取事件，将 `<releases>` 之前和之后的内容直接写入 Writer，
    /// 当遇到 `<releases>` 时，调用 `handle_releases` 进行处理。
    ///
    /// # Returns
    ///
    /// * `error::Result<()>` - 成功或失败。
    fn start(&mut self) -> error::Result<()> {
        // 循环读取 XML 事件
        loop {
            match self.reader.read_event() {
                // 匹配到 <releases> 开始标签
                Ok(Event::Start(e)) if e.name().as_ref() == b"releases" => {
                    // 将 <releases> 开始标签写入到输出 Writer
                    self.writer
                        .write_event(Event::Start(e.to_owned())) // 需要克隆以获得所有权
                        .with_whatever_context(|e| format!("Failed to write <releases> start tag: {:?}", e))?;
                    // 调用 handle_releases 来处理 <releases> 标签内部的内容
                    self.handle_releases()?;
                }
                // 匹配到文件结束符 (EOF)
                Ok(Event::Eof) => {
                    // 正常结束处理
                    return Ok(());
                }
                // 匹配到任何其他事件（例如 XML 声明、注释、<releases> 之外的元素等）
                Ok(e) => {
                    // 将这些事件原样写入到输出 Writer
                    self.writer
                        .write_event(e) // 注意：这里的 e 是借用的，但 write_event 可以处理
                        .with_whatever_context(failed_to_write_event)?
                }
                // 处理读取事件时发生的错误
                Err(e) => {
                    // 使用宏创建并返回错误
                    return handle_read_event_err!(self, e);
                }
            }
        }
    }
}

/// 公开函数，用于更新指定路径的 XML 文件。
///
/// # Arguments
///
/// * `path` - 要更新的 XML 文件的路径。
/// * `vec` - 一个包含新产品发布信息的可变 Vec 的引用。
///
/// # Returns
///
/// * `error::Result<()>` - 成功或失败。
pub fn update_xml(path: String, vec: &mut Vec<ProductRelease>) -> error::Result<()> {
    // 读取指定路径的整个 XML 文件内容到字节向量中
    let xml_bytes = std::fs::read(&path)
        .with_whatever_context(|x| format!(r#"Failed to read "{}", source: {}"#, path, x))?;

    // 从读取到的字节 slice 创建一个 quick_xml Reader
    let mut reader = Reader::from_reader(xml_bytes.as_slice());
    // 配置 Reader：不自动去除文本事件前后的空白字符
    // 这对于保留 XML 的原始格式很重要
    reader.config_mut().trim_text(false);

    // 创建 XMLHandler 实例，初始化所有字段
    let mut handler = XMLHandler {
        path: &path, // 传入文件路径引用
        writer: Writer::new(Cursor::new(Vec::new())), // 创建一个新的 Writer，写入到内存中的 Vec<u8>
        reader, // 传入创建的 Reader
        vec, // 传入新发布信息的可变引用
        preserved_xml_fragments: vec![], // 初始化空的保留片段列表
    };

    // 调用 handler 的 start 方法开始处理
    handler.start()?;

    // 处理完成后，获取 handler 内部 writer 所写入的全部字节内容
    let output_bytes = handler.writer.into_inner().into_inner();
    // 将这些字节内容写回到原始文件路径，覆盖原文件
    std::fs::write(&path, output_bytes)
        .with_whatever_context(|x| format!(r#"Failed to write "{}", source: {}"#, path, x))?;

    // 一切顺利，返回 Ok
    Ok(())
}