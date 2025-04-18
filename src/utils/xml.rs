use crate::resolve::ProductRelease;
use quick_xml::events::attributes::{Attribute, Attributes};
use quick_xml::events::{BytesEnd, Event};
use quick_xml::{Reader, Writer};
use std::borrow::Cow;
use std::io::Cursor;

trait XMLWriter {
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>);

    fn date(&self) -> &str;
}

impl XMLWriter for ProductRelease<'_> {
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        writer
            .create_element("release")
            .with_attribute(Attribute::from(("version", self.version)))
            .with_attribute(Attribute::from(("date", self.date)))
            .write_empty()
            .unwrap();
    }

    fn date(&self) -> &str {
        &self.date
    }
}

impl XMLWriter for (String, Vec<Event<'_>>) {
    fn write_xml(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        for event in self.1.iter() {
            writer.write_event(event.clone()).unwrap();
        }
    }

    fn date(&self) -> &str {
        self.0.as_str()
    }
}

struct XMLHandler<'a, 'b, 'c> {
    writer: Writer<Cursor<Vec<u8>>>,
    reader: Reader<&'a [u8]>,
    vec: &'c mut Vec<ProductRelease<'b>>,
    exits: Vec<(String, Vec<Event<'a>>)>,
}

impl<'a, 'b, 'c> XMLHandler<'a, 'b, 'c> {
    fn handle_end_of_releases(&mut self, e: BytesEnd) {
        self.writer.write_bom().unwrap();
        
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
            xw.write_xml(&mut self.writer);
        }
        assert!(self.writer.write_event(Event::End(e)).is_ok());
    }

    fn handle_releases(&mut self) {
        fn search_date<'a>(
            mut attr: Attributes<'a>,
            vec: &mut Vec<ProductRelease>,
        ) -> (bool, Cow<'a, [u8]>) {
            let value = attr
                .find(|attr| attr.as_ref().unwrap().key.0.eq("date".as_bytes()))
                .unwrap()
                .unwrap()
                .value;

            (
                vec.iter().any(|r| r.date.as_bytes().eq(value.as_ref())),
                value,
            )
        }
        let mut is_skip_release = false;
        loop {
            match self.reader.read_event() {
                Ok(Event::End(e)) if e.name().as_ref() == b"releases" => {
                    self.handle_end_of_releases(e);
                    break;
                }
                Ok(Event::Eof) => panic!("EOF found"),
                Ok(Event::Start(e)) if e.name().as_ref() == b"release" => {
                    is_skip_release = false;
                    let (is_exist, value) = search_date(e.attributes(), self.vec);
                    if is_exist {
                        self.exits.push((
                            String::from_utf8(value.as_ref().to_vec()).unwrap(),
                            vec![Event::Start(e.to_owned())],
                        ));
                    } else {
                        is_skip_release = true;
                    }
                }
                Ok(Event::Empty(e)) if e.name().as_ref() == b"release" => {
                    let (is_exist, value) = search_date(e.attributes(), self.vec);
                    if is_exist {
                        self.exits.push((
                            String::from_utf8(value.as_ref().to_vec()).unwrap(),
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
                            .unwrap()
                            .1
                            .push(Event::End(e.to_owned()));
                    } else {
                        is_skip_release = false;
                    }
                }
                Ok(e) => {
                    if !is_skip_release { // 如果未跳过，则推入最新的exits内
                        if let Some(last) = self.exits.last_mut() {
                            last.1.push(e);
                        }
                    }
                }
                Err(e) => panic!(
                    "Error at position {}: {:?}",
                    self.reader.error_position(),
                    e
                ),
            }
        }
    }

    fn start(&mut self) {
        loop {
            match self.reader.read_event() {
                Ok(Event::Start(e)) if e.name().as_ref() == b"releases" => {
                    self.writer.write_event(Event::Start(e.to_owned())).unwrap();
                    self.handle_releases();
                }
                Ok(Event::Eof) => break,
                Ok(e) => assert!(self.writer.write_event(e).is_ok()),
                Err(e) => panic!(
                    "Error at position {}: {:?}",
                    self.reader.error_position(),
                    e
                ),
            }
        }
    }
}

pub fn update_xml(path: String, vec: &mut Vec<ProductRelease>) {
    let xml = std::fs::read_to_string(&path).unwrap();
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(true);
    let mut handler = XMLHandler {
        writer: Writer::new(Cursor::new(Vec::new())),
        reader,
        vec,
        exits: vec![],
    };
    handler.start();
    if !handler.vec.is_empty() { // 说明xml已经过时了
        std::fs::write(path, handler.writer.into_inner().into_inner()).unwrap();
    }
}
