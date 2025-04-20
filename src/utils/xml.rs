use crate::error;
use crate::resolve::ProductRelease;
use crate::utils::xml::_err::failed_to_write_event;
use quick_xml::events::attributes::{Attribute, Attributes};
use quick_xml::events::{BytesEnd, Event};
use quick_xml::{Reader, Writer};
use snafu::{OptionExt, ResultExt, whatever};
use std::borrow::Cow;
use std::io::Cursor;

mod _err {
    #[inline]
    pub fn failed_to_write_event(e: &mut std::io::Error) -> String {
        format!("Failed to write event to XML: {:?}", e)
    }
}

trait XMLWriter {
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> error::Result<()>;

    fn date(&self) -> &str;
}

impl XMLWriter for ProductRelease<'_> {
    #[inline]
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> error::Result<()> {
        writer
            .create_element("release")
            .with_attribute(Attribute::from(("version", self.version)))
            .with_attribute(Attribute::from(("date", self.date)))
            .write_empty()
            .with_whatever_context(failed_to_write_event)?;
        Ok(())
    }

    #[inline]
    fn date(&self) -> &str {
        &self.date
    }
}

impl XMLWriter for (String, Vec<Event<'_>>) {
    #[inline]
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> error::Result<()> {
        for event in self.1.iter() {
            writer
                .write_event(event.clone())
                .with_whatever_context(failed_to_write_event)?;
        }
        Ok(())
    }

    #[inline]
    fn date(&self) -> &str {
        self.0.as_str()
    }
}

struct XMLHandler<'a, 'b, 'c> {
    path: &'a str,
    writer: Writer<Cursor<Vec<u8>>>,
    reader: Reader<&'a [u8]>,
    vec: &'c mut Vec<ProductRelease<'b>>,
    exits: Vec<(String, Vec<Event<'a>>)>,
}

macro_rules! handle_read_event_err {
    ($s:ident,$e:ident) => {
        whatever!(
            r#"Error happen at position {} while parsing XML in "{}", source: {:?}"#,
            $s.reader.error_position(),
            $s.path,
            $e
        )
    };
}

impl<'a, 'b, 'c> XMLHandler<'a, 'b, 'c> {
    fn handle_end_of_releases(&mut self, e: BytesEnd) -> error::Result<()> {
        self.writer
            .write_bom()
            .with_whatever_context(failed_to_write_event)?;

        // vec保留exits(已在xml中出现的)中未出现的
        self.vec
            .retain(|x| !self.exits.iter().any(|y| x.date.eq(&y.0)));

        let mut vec_new: Vec<&dyn XMLWriter> = Vec::from_iter(
            self.vec
                .iter()
                .map(|x| x as &dyn XMLWriter)
                .chain(self.exits.iter().map(|e| e as &dyn XMLWriter)),
        );
        vec_new.sort_unstable_by_key(|e| e.date());

        for xw in vec_new.iter().rev() {
            xw.write_xml(&mut self.writer)?;
        }
        self.writer
            .write_event(Event::End(e))
            .with_whatever_context(failed_to_write_event)
    }

    fn handle_releases(&mut self) -> error::Result<()> {
        fn search_date<'a>(
            mut attr: Attributes<'a>,
            vec: &mut Vec<ProductRelease>,
        ) -> error::Result<(bool, Cow<'a, [u8]>)> {
            let value = attr
                .find_map(|x| {
                    x.with_whatever_context(|e| {
                        format!("Failed to parse release tag's attribute, source: {:?}", e)
                    })
                    .map(|attr| attr.key.0.eq(b"date").then_some(attr.value))
                    .transpose()
                })
                .whatever_context("Failed to find date attribute in release tag")??;

            Ok((
                vec.iter().any(|r| r.date.as_bytes().eq(value.as_ref())),
                value,
            ))
        }
        let mut is_skip_release = false;
        loop {
            match self.reader.read_event() {
                Ok(Event::End(e)) if e.name().as_ref() == b"releases" => {
                    return self.handle_end_of_releases(e);
                }
                Ok(Event::Eof) => whatever!(
                    r#"Unexpected EOF found in releases tag while parsing XML in XMLWriter "{}""#,
                    self.path
                ),
                Ok(Event::Start(e)) if e.name().as_ref() == b"release" => {
                    is_skip_release = false;
                    let (is_exist, value) = search_date(e.attributes(), self.vec)?;
                    if is_exist {
                        self.exits.push((
                            String::from_utf8(value.as_ref().to_vec())
                                .whatever_context("Failed to convert string to UTF-8 after search_date, this is a bug, please report it and post logs.")?,
                            vec![Event::Start(e.to_owned())],
                        ));
                    } else {
                        is_skip_release = true;
                    }
                }
                Ok(Event::Empty(e)) if e.name().as_ref() == b"release" => {
                    let (is_exist, value) = search_date(e.attributes(), self.vec)?;
                    if is_exist {
                        self.exits.push((
                            String::from_utf8(value.as_ref().to_vec())
                                .whatever_context("Failed to convert string to UTF-8 after search_date, this is a bug, please report it and post logs.")?,
                            vec![Event::Empty(e.to_owned())],
                        ));
                    } else {
                        is_skip_release = true;
                    }
                }
                Ok(Event::End(e)) if e.name().as_ref() == b"release" => {
                    if !is_skip_release {
                        self.exits
                            .last_mut()
                            .whatever_context("Failed to find last mut in exits, this is a bug, please report it and post logs.")?
                            .1
                            .push(Event::End(e.to_owned()));
                    } else {
                        is_skip_release = false;
                    }
                }
                Ok(e) => {
                    if !is_skip_release {
                        // 如果未跳过，则推入最新的exits内
                        if let Some(last) = self.exits.last_mut() {
                            last.1.push(e);
                        }
                    }
                }
                Err(e) => handle_read_event_err!(self, e),
            }
        }
    }

    fn start(&mut self) -> error::Result<()> {
        loop {
            match self.reader.read_event() {
                Ok(Event::Start(e)) if e.name().as_ref() == b"releases" => {
                    self.writer
                        .write_event(Event::Start(e.to_owned()))
                        .with_whatever_context(|e| format!("{:?}", e))?;
                    self.handle_releases()?;
                }
                Ok(Event::Eof) => return Ok(()),
                Ok(e) => self
                    .writer
                    .write_event(e)
                    .with_whatever_context(failed_to_write_event)?,
                Err(e) => handle_read_event_err!(self, e),
            }
        }
    }
}

pub fn update_xml(path: String, vec: &mut Vec<ProductRelease>) -> error::Result<()> {
    let xml = std::fs::read_to_string(&path)
        .with_whatever_context(|x| format!(r#"Failed to read "{}", source: {}"#, path, x))?;
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(true);
    let mut handler = XMLHandler {
        path: &path,
        writer: Writer::new(Cursor::new(Vec::new())),
        reader,
        vec,
        exits: vec![],
    };
    handler.start()?;
    if !handler.vec.is_empty() {
        // 说明xml已经过时了
        std::fs::write(&path, handler.writer.into_inner().into_inner())
            .with_whatever_context(|x| format!(r#"Failed to write "{}", source: {}"#, path, x))?;
    }
    Ok(())
}
