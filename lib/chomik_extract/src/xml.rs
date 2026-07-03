use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;
use std::str::FromStr;

// use serde::Deserialize;
use xml::EventReader;
use xml::reader::XmlEvent;

use crate::AnimFile;

pub struct AnimContents<'anim_file> {
    pub name: Box<str>,
    pub animations: Box<[Animation]>,
    pub jpegs: HashMap<&'anim_file str, Rc<[u8]>>,
}

const XML_MAGICS: [&[u8]; 2] = [b"<?xml", b"\xEF\xBB\xBF<?xml"];
const JPEG_MAGICS: [&[u8]; 3] = [
    [0xFF, 0xD8, 0xFF].as_slice(),
    [0x00, 0x00, 0x00, 0x0C, 0x6A, 0x50, 0x20, 0x20, 0x0D, 0x0A, 0x87, 0x0A].as_slice(),
    [0xFF, 0x4F, 0xFF, 0x51].as_slice(),
];

pub fn extract_files(extract_from: &AnimFile) -> Result<AnimContents<'_>, ()> {
    enum FileType {
        Jpeg,
        Xml,
    }

    let (xml, mut jpegs) = extract_from
        .files()
        .into_iter()
        .map(|f| {
            let data = f.decompressed_data().unwrap();
            if XML_MAGICS.iter().any(|magic| data.starts_with(magic)) {
                Ok((FileType::Xml, f.name(), data))
            } else if JPEG_MAGICS.iter().any(|magic| data.starts_with(magic)) {
                Ok((FileType::Jpeg, f.name(), data))
            } else {
                dbg!(data);
                Err(())
            }
        })
        .try_fold((None, HashMap::<&str, Rc<[u8]>>::new()), |(mut xml, mut jpegs): (_, _), v| {
            let (ft, filename, data) = v?;
            match ft {
                FileType::Jpeg => {
                    jpegs.insert(filename, Rc::from(data));
                }
                FileType::Xml => {
                    if let Some((prev_filename, _)) = xml {
                        panic!(
                            "`.anim` file contains multiple XML files (tried to overwrite '{prev_filename}' with \
                             '{filename}')",
                        )
                    }
                    // println!("```\n{}\n```", str::from_utf8(data.as_ref()).unwrap());

                    xml = Some((filename, data));
                }
            }
            Ok((xml, jpegs))
        })?;

    let (_, data) = xml.unwrap();
    let (name, animations) = self::parse(data.as_ref()).unwrap();

    Ok(AnimContents { name, animations, jpegs })
}

// #[derive(Debug)]
pub struct Animation {
    pub name: Box<str>,
    pub conditions: Conditions,
    pub way: Way,
    pub files: Box<[Box<str>]>,
}

impl std::fmt::Debug for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animation")
            .field("name", &self.name)
            .field("conditions", &self.conditions)
            .field("way", &self.way)
            .finish_non_exhaustive()
    }
}

macro_rules! dbg_inline {
    (impl $color:ident, none for $struct:ident { $($field:ident),+ }) => {
        impl $struct {
            pub fn dbg_inline(&self) -> String {
                dbg_inline!(
                    self,
                    none,
                    $($field),+
                )
            }

            pub fn dbg_inline_clr(&self) -> String {
                dbg_inline!(
                    self,
                    $color,
                    $($field),+
                )
            }
        }
    };

    ($self:expr, red, $($field:ident),+) => {
        dbg_inline!(@ $self, 9, $($field),+)
    };
    ($self:expr, green, $($field:ident),+) => {
        dbg_inline!(@ $self, 10, $($field),+)
    };
    ($self:expr, none, $($field:ident),+) => {
        dbg_inline!(@@ $self, "", "", "", $($field),+)
    };

    (@ $self:expr, $color:literal, $($field:ident),+) => {
        dbg_inline!(@@ $self, concat!("\u{001B}[38;5;", stringify!($color), "m"), "\u{001B}[38;5;244m", "\u{001B}[0m", $($field),+)
    };

    (@@ $self:expr, $color:expr, $gray:expr, $reset:expr, $($field:ident),+) => {{
        let mut out = concat!($color, "(").to_string();

        $(
            if let Some(value) = $self.$field.as_ref() {
                out.push_str(format!(concat!(stringify!($field), $gray, ": {}, ", $color), value).as_ref());
            }
        )+
        if out.ends_with(concat!(", ", $color)) {
            out.truncate(out.len() - concat!(", ", $color).len());
            out.push_str($color);
        }

        out.push_str(concat!(")", $reset));
        out
    }};
}

macro_rules! apply_attributes {
    ($out:expr, $attr:expr, [$(
        $xml_attr_name:literal => $struct_attr_name:ident,
    )+]) => {
        match $attr.name.local_name.as_str() {
            $($xml_attr_name => {
                $out.$struct_attr_name = Some($attr.value.parse().unwrap());
                None
            })+
            _ => Some(($attr.name.local_name, $attr.value)),
        }
    };
}

#[derive(Debug, Default)]
pub struct Conditions {
    pub idle: Option<bool>,
    pub mouse_press: Option<bool>,
    pub file_over: Option<bool>,
    pub file_drop: Option<bool>,
    pub duration: Option<u64>,
    pub exit_immediately: Option<bool>,
    pub player_playing: Option<bool>,
    pub screenshot: Option<bool>,
    pub typing: Option<bool>,
    pub probability: Option<u64>,
    pub priority: Option<u64>,
}

dbg_inline!(impl red, none for Conditions {
    idle,
    mouse_press,
    file_over,
    file_drop,
    duration,
    exit_immediately,
    player_playing,
    screenshot,
    typing,
    probability,
    priority
});

impl Conditions {
    fn apply_attribute(&mut self, attribute: xml::attribute::OwnedAttribute) -> Result<(), (String, String)> {
        apply_attributes!(self, attribute, [
            "idle" => idle,
            "mousePress" => mouse_press,
            "exitImmediately" => exit_immediately,
            "probability" => probability,
            "priority" => priority,
            "file-over" => file_over,
            "file-drop" => file_drop,
            "duration" => duration,
            "player-playing" => player_playing,
            "screenshot" => screenshot,
            "typing" => typing,
        ])
        .map_or(Ok(()), Err)
    }

    fn from_attributes(
        attributes: impl IntoIterator<Item = xml::attribute::OwnedAttribute>,
    ) -> Result<Self, (String, String)> {
        let mut out = Self::default();

        attributes.into_iter().try_for_each(|a| out.apply_attribute(a)).map(|()| out)
    }
}

#[derive(Debug, Default)]
pub struct Way {
    // Seemingly not optional.
    pub start: Option<State>,
    // Seemingly not optional.
    pub stop: Option<State>,
    pub enter: Option<bool>,
    pub exit: Option<bool>,
    pub prob: Option<u64>,
}

dbg_inline!(impl green, none for Way { start, stop, enter, exit, prob });

impl Way {
    fn apply_attribute(&mut self, attribute: xml::attribute::OwnedAttribute) -> Result<(), (String, String)> {
        apply_attributes!(self, attribute, [
            "start" => start,
            "stop" => stop,
            "enter" => enter,
            "exit" => exit,
            "prob" => prob,
        ])
        .map_or(Ok(()), Err)
    }

    fn from_attributes(
        attributes: impl IntoIterator<Item = xml::attribute::OwnedAttribute>,
    ) -> Result<Self, (String, String)> {
        let mut out = Self::default();

        attributes.into_iter().try_for_each(|a| out.apply_attribute(a)).map(|()| out)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum State {
    State1,
    State2,
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::State1 => "State1",
            Self::State2 => "State2",
        })
    }
}

impl FromStr for State {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "State1" => Ok(Self::State1),
            "State2" => Ok(Self::State2),
            _ => Err(()),
        }
    }
}

fn parse<R: Read>(xml_file: R) -> Result<(Box<str>, Box<[Animation]>), xml::reader::Error> {
    let config = xml::ParserConfig::new()
        .trim_whitespace(true)
        .cdata_to_characters(true)
        .ignore_comments(true)
        .coalesce_characters(true);
    let parser = EventReader::new_with_config(xml_file, config);

    let mut in_opml = false;
    let mut in_package = false;
    let mut in_file = false;

    let mut package_count = 0;

    let mut package_name = None;
    let mut animations = Vec::new();
    let mut current_animation: Option<(Box<str>, Conditions, Way, Vec<Box<str>>)> = None;

    for event in parser {
        match event? {
            XmlEvent::StartElement { name, attributes, .. } => match name.local_name.as_str() {
                "file" => in_file = true,
                "opml" => in_opml = true,
                "package" => {
                    if !in_opml {
                        panic!();
                    }

                    in_package = true;
                    package_count += 1;
                    package_name = Some(
                        attributes.into_iter().find(|a| a.name.local_name == "name").unwrap().value.into_boxed_str(),
                    );
                }
                "animation" => {
                    if !in_package {
                        panic!();
                    }

                    current_animation = Some((
                        attributes.into_iter().find(|a| a.name.local_name == "name").unwrap().value.into_boxed_str(),
                        Conditions::default(),
                        Way::default(),
                        Vec::new(),
                    ));
                }
                "conditions" => {
                    current_animation.as_mut().unwrap().1 = Conditions::from_attributes(attributes).unwrap();
                }
                "way" => {
                    current_animation.as_mut().unwrap().2 = Way::from_attributes(attributes).unwrap();
                }
                other => panic!("unexpected element: {other}"),
            },
            XmlEvent::EndElement { name } => match name.local_name.as_str() {
                "file" => in_file = false,
                "opml" => {
                    in_opml = false;
                }
                "package" => {
                    in_package = false;
                }
                "animation" => {
                    if let Some((name, conditions, way, files)) = current_animation.take() {
                        animations.push(Animation { name, conditions, way, files: files.into_boxed_slice() });
                    } else {
                        panic!()
                    }
                }
                "conditions" | "way" => (),
                _ => panic!(),
            },
            XmlEvent::Characters(str) => {
                if in_file {
                    current_animation.as_mut().unwrap().3.push(str.into_boxed_str());
                } else {
                    panic!();
                }
            }
            XmlEvent::StartDocument { .. } | XmlEvent::EndDocument => (),
            _ => panic!(),
        }
    }

    if package_count != 1 {
        panic!("expected 1 package, got {package_count}");
    }

    Ok((package_name.unwrap(), animations.into_boxed_slice()))
}

// #[derive(Deserialize)]
// struct OpmlFile {
//     // TO-DO: there may be multiple per OPML file.
//     package: OpmlPackage,
// }
//
// #[derive(Deserialize)]
// struct OpmlPackage {
//     animations: Box<[OpmlAnimation]>,
// }
//
// #[derive(Deserialize)]
// struct OpmlAnimation {
//     conditions: (),
//     way: (),
//     files: Box<[()]>,
// }
