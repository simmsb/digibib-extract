use std::num::{NonZeroU8, NonZeroU16};

use std::fmt::Write;

use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Style {
    pub left_padding: Option<NonZeroU16>,
    pub emphasis: bool,
    pub strong: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub strikethrough: bool,
    pub underline: bool,
    pub wide_spacing: bool,
    pub size: Option<NonZeroU8>,
    pub color_gray: bool,
    pub no_justification: bool,
    pub alignment: Option<&'static str>,
}

pub trait Encoder {
    fn chunk(&mut self, s: &str, style: &Style);
    fn link(&mut self, url: &str, content: &str);
    fn pageref(&mut self, page: u32);
    fn searchword(&mut self, s: &str);
}

use crate::{decoding, toc::TocItem, token::Token};

struct State<'a, E> {
    encoder: &'a mut E,
    queued_link: Option<(String, String)>,
    font_idx: u8,
    word_incomplete: bool,
    had_carriage_return: bool,
    add_hyphen_at_eol: bool,
    add_hyphen_at_eol_separating_ck: bool,
    add_invisible_hyphen: bool,
    file_name: Option<String>,
    concordance: Option<u16>,
    node_number: Option<u16>,
    sigil: Option<String>,
    current_style: Style,
}

impl<'a, E: Encoder> State<'a, E> {
    fn new(encoder: &'a mut E) -> Self {
        Self {
            encoder,
            queued_link: None,
            font_idx: 0,
            word_incomplete: false,
            had_carriage_return: false,
            add_hyphen_at_eol: false,
            add_hyphen_at_eol_separating_ck: false,
            add_invisible_hyphen: false,
            file_name: None,
            concordance: None,
            node_number: None,
            sigil: None,
            current_style: Default::default(),
        }
    }

    fn reset_hyphens(&mut self) {
        self.add_hyphen_at_eol = false;
        self.add_hyphen_at_eol_separating_ck = false;
        self.add_invisible_hyphen = false;
    }

    fn hyphen(&self) -> bool {
        self.add_hyphen_at_eol || self.add_hyphen_at_eol_separating_ck || self.add_invisible_hyphen
    }
}

impl<'a, E: Encoder> Write for State<'a, E> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        if let Some(link) = &mut self.queued_link {
            link.0.push_str(s);
            Ok(())
        } else {
            self.encoder.chunk(s, &self.current_style);
            Ok(())
        }
    }
}


pub fn encode_page(
    tocitem: &TocItem,
    page_number: usize,
    lexed: &[Token],
    encoder: &mut impl Encoder,
) -> eyre::Result<()> {
    let mut state = State::new(encoder);

    for t in lexed {
        match t {
            Token::Blanks(number) => {
                for _ in 0..*number {
                    write!(state, " ")?;
                }
            }
            Token::Word { space_at_end, data } => {
                let s = decoding::decode_string(data, state.font_idx);

                let s = if !state.hyphen() {
                    s.trim_end().trim_end_matches('-')
                } else {
                    s.trim_end()
                };

                if state.word_incomplete {
                    state.word_incomplete = false;
                } else {
                    if s.len() > 0 {
                        write!(state, "{}", s)?;
                    }
                }

                state.reset_hyphens();
                if *space_at_end
                    || s.chars()
                        .next_back()
                        .map_or(false, |c| !c.is_alphanumeric())
                {
                    write!(state, " ")?;
                }
            }
            Token::HardCarriageReturn => {
                state.had_carriage_return = true;
                writeln!(state, "\n")?;
            }
            Token::EndOfPage => {
                break;
            }
            Token::ItalicsOn => {
                state.current_style.emphasis = true;
            }
            Token::ItalicsOff => {
                state.current_style.emphasis = false;
            }
            Token::BoldOn => {
                state.current_style.strong = true;
            }
            Token::BoldOff => {
                state.current_style.strong = false;
            }
            Token::FontPreset(n) => match n {
                0 => {
                    state.current_style.color_gray = false;
                    state.current_style.emphasis = false;
                    state.current_style.strong = false;
                }
                1 => {
                    state.current_style.size = Some(NonZeroU8::new(133).unwrap());
                }
                2 => {
                    state.current_style.size = Some(NonZeroU8::new(122).unwrap());
                }
                3 => {
                    state.current_style.size = Some(NonZeroU8::new(111).unwrap());
                }
                4 => {
                    state.current_style.size = None;
                    state.current_style.strong = true;
                }
                5 => {
                    state.current_style.size = None;
                }
                6 => {
                    state.current_style.size = None;
                    state.current_style.emphasis = true;
                }
                _ => {}
            },
            Token::Ly => {
                // ???
            }
            Token::Image { width, name } => {}
            Token::ImageLink(_) => {}
            Token::EndLink => {}
            Token::Font(n) => {
                state.font_idx = *n;
            }
            Token::FileName(s) => {
                state.file_name = Some(s.data.to_owned());
            }
            Token::Concordance(n) => {
                state.concordance = Some(*n);
            }
            Token::NodeNumber(n) => {
                state.node_number = Some(*n);
            }
            Token::SuperScriptOn => {
                state.current_style.superscript = true;
            }
            Token::SuperScriptOff => {
                state.current_style.superscript = false;
            }
            Token::Sigil(s) => {
                state.sigil = Some(s.data.clone());
            }
            Token::Header => {}
            Token::HypenAtEol => {
                state.add_invisible_hyphen = true;
            }
            Token::UnderlineOn => {
                state.current_style.underline = true;
            }
            Token::UnderlineOff => {
                state.current_style.underline = false;
            }
            Token::GreekOn => {}
            Token::GreekOff => {}
            Token::OneBlank => {
                write!(state, " ")?;
            }
            Token::VerticalLineOn => {
                // not used
            }
            Token::VerticalLineOff => {}
            Token::TD => {}
            Token::Null => {}
            Token::PageLink { page_number, name } => {
                if *page_number != 0 {
                    state.encoder.pageref(*page_number);
                } else {
                    // TODO image link
                }
            }
            Token::IDStart(_) => {}
            Token::IDEnd(_) => {}
            Token::SubscriptOn => {
                state.current_style.subscript = true;
            }
            Token::SubscriptOff => {
                state.current_style.subscript = false;
            }
            Token::Color(colour) => {
                if *colour == 1 {
                    state.current_style.color_gray = true;
                } else {
                    // yes, I know
                    state.current_style.color_gray = false;
                }
            }
            Token::InlineImage {
                width,
                height,
                name,
            } => {}
            Token::SearchWord(_) => {}
            Token::FontSize(size) => {
                state.current_style.size = Some(NonZeroU8::new(*size).unwrap());
            }
            Token::Copyright(_) => {}
            Token::AutoLink(page) => {
                state.encoder.pageref(*page);
            }
            Token::SoftCarriageReturn => {
                if !state.hyphen() {
                    write!(state, " ")?;
                }
            }
            Token::InvisibleHyphen => {
                state.add_invisible_hyphen = true;
            }
            Token::LetterSpacingOn => {
                state.current_style.wide_spacing = true;
            }
            Token::LetterSpacingOff => {
                state.current_style.wide_spacing = false;
            }
            Token::HalfLineSpacing => {
                write!(state, "\n")?;
            }
            Token::ListItemStart => {
                panic!("actuall saw a list item");
            }
            Token::ListItemEnd => {
            }
            Token::UnorderedListStart => {}
            Token::UnorderedListEnd => {}
            Token::SetX(indent) => {
                state.current_style.left_padding = NonZeroU16::new(*indent);
            }
            Token::SV(_) => {}
            Token::SVLemmaBegin(_) => {}
            Token::SVLemmaStop => {}
            Token::CenteredOn => {
                state.current_style.alignment = Some("center");
            }
            Token::CenteredOff => {
                state.current_style.alignment = None;
            }
            Token::AlignRightOn => {
                state.current_style.alignment = Some("right");
            }
            Token::AlignRightOff => {
                state.current_style.alignment = None;
            }
            Token::EOn => {}
            Token::EOff => {}
            Token::BibIndex(_) => {}
            Token::NotFirstLine => {}
            Token::Thumb => {}
            Token::EndNew(_) => {}
            Token::UrlBegin(url) => {
                state.queued_link = Some((String::new(), url.data.to_owned()));
            }
            Token::UrlEnd => {
                if let Some((content, url)) = state.queued_link.take() {
                    state.encoder.link(&url, &content);
                }
            }
            Token::WordAnchor => {}
            Token::ThumbWWW => {}
            Token::S => {}
            Token::NoJustifyOn => {
                state.current_style.no_justification = true;
            }
            Token::NoJustifyOff => {
                state.current_style.no_justification = false;
            }
            Token::NextBlankFixed => {}
            Token::WordRest { space_at_end, data } => {
                write!(state, "{}", data)?;

                if *space_at_end {
                    write!(state, " ")?;
                }
            }
            Token::WordIncomplete(word) => {
                write!(state, "{}", word.data)?;
                state.word_incomplete = true;
            }
            Token::HyphenCK => {
                state.add_hyphen_at_eol_separating_ck = true;
            }
            Token::HebrewOn => {}
            Token::HebrewOff => {}
            Token::NodeNumber2(_) => {}
            Token::StrikeThroughOn => {
                state.current_style.strikethrough = true;
            }
            Token::StrikeThroughOff => {
                state.current_style.strikethrough = false;
            }
            Token::SetY(_) => {}
            Token::Cor(_) => {}
            Token::EndCor => {}
            Token::DashedLine => {}
            Token::Unknown { raw, decoded } => {}
        }
    }

    Ok(())
}
