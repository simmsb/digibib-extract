use std::{fmt::Write, collections::{HashMap, HashSet}};

use once_cell::sync::{OnceCell, Lazy};
use regex::Regex;

use crate::{decoding, text::Page, token::Token};

pub struct State<W> {
    writer: W,
    superscript: bool,
    font_idx: u8,
    word_incomplete: bool,
    had_carriage_return: bool,
    add_hyphen_at_eol: bool,
    add_hyphen_at_eol_separating_ck: bool,
    add_invisible_hyphen: bool,
    file_name: Option<String>,
    concordance: Option<u16>,
    node_number: Option<u16>,
    current_functions: HashSet<&'static str>,
}

impl<W: Write> State<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            superscript: false,
            font_idx: 0,
            word_incomplete: false,
            had_carriage_return: false,
            add_hyphen_at_eol: false,
            add_hyphen_at_eol_separating_ck: false,
            add_invisible_hyphen: false,
            file_name: None,
            concordance: None,
            node_number: None,
            current_functions: HashSet::new(),
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

    fn push_state(&mut self, key: &'static str, val: impl AsRef<str>) -> eyre::Result<()> {
        if !self.current_functions.insert(key) {
            write!(self, "]")?;
        }

        write!(self, "#{}[", val.as_ref())?;

        Ok(())
    }

    fn pop_state(&mut self, key: &str) -> eyre::Result<()> {
        if self.current_functions.remove(key) {
            write!(self, "]")?;
        }

        Ok(())
    }
}

impl<W: Write> Write for State<W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.writer.write_str(s)
    }
}

pub fn write_page(page: &Page, lexed: &[Token], output: impl Write) -> eyre::Result<()> {
    let mut state = State::new(output);

    for t in lexed {
        match t {
            Token::Blanks(number) => {
                for _ in 0..*number {
                    write!(state, " ")?;
                }
            }
            Token::Word { space_at_end, data } => {
                let s = decoding::decode_string(data, state.font_idx);

                if state.word_incomplete {
                    state.word_incomplete = false;
                } else {
                    if s.len() > 0 {

                        static ESCAPER: Lazy<Regex> = Lazy::new(|| Regex::new(r"[#()\[\]]").unwrap());

                        write!(state, "{}", ESCAPER.replace_all(&s, "\\$0"))?;
                    }
                }

                state.reset_hyphens();
                if *space_at_end || s.chars().next_back().map_or(false, |c| !c.is_alphanumeric()) {
                    write!(state, " ")?;
                }

            }
            Token::HardCarriageReturn => {
                state.had_carriage_return = true;
                writeln!(state, "\\")?;
            }
            Token::EndOfPage => { break; }
            Token::ItalicsOn => { state.push_state("emph", "emph")?; }
            Token::ItalicsOff => { state.pop_state("emph")?; }
            Token::BoldOn => { state.push_state("strong", "strong")?; }
            Token::BoldOff => { state.pop_state("strong")?; }
            Token::FontPreset(n) => {
                match n {
                    0 => {
                        state.push_state("font", format!("text(size: {}em)", 1.0))?;
                        state.pop_state("strong")?;
                        state.pop_state("italic")?;
                    }
                    1 => {
                        state.push_state("font", format!("text(size: {}em)", 1.33))?;
                    }
                    2 => {
                        state.push_state("font", format!("text(size: {}em)", 1.22))?;
                    }
                    3 => {
                        state.push_state("font", format!("text(size: {}em)", 1.11))?;
                    }
                    4 => {
                        state.push_state("font", format!("text(size: {}em)", 1.0))?;
                        state.push_state("strong", "strong")?;
                    }
                    5 => {
                        state.push_state("font", format!("text(size: {}em)", 1.0))?;
                    }
                    6 => {
                        state.push_state("font", format!("text(size: {}em)", 1.0))?;
                        state.push_state("emph", "emph")?;
                    }
                    _ => {}
                }
            }
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
                state.superscript = true;
            }
            Token::SuperScriptOff => {
                state.superscript = false;
            }
            Token::Sigil(_) => {}
            Token::Header => {}
            Token::HypenAtEol => {}
            Token::UnderlineOn => {}
            Token::UnderlineOff => {}
            Token::GreekOn => {}
            Token::GreekOff => {}
            Token::OneBlank => {}
            Token::VerticalLineOn => {}
            Token::VerticalLineOff => {}
            Token::TD => {}
            Token::Null => {}
            Token::PageLink { page_number, name } => {}
            Token::IDStart => {}
            Token::IDEnd => {}
            Token::SubscriptOn => {}
            Token::SubscriptOff => {}
            Token::Color(_) => {}
            Token::InlineImage {
                width,
                height,
                name,
            } => {}
            Token::SearchWord(_) => {}
            Token::FontSize(_) => {}
            Token::Copyright(_) => {}
            Token::AutoLink(_) => {}
            Token::SoftCarriageReturn => {}
            Token::InvisibleHyphen => {}
            Token::LetterSpacingOn => {}
            Token::LetterSpacingOff => {}
            Token::HalfLineSpacing => {}
            Token::ListItemStart => {}
            Token::ListItemEnd => {}
            Token::UnorderedListStart => {}
            Token::UnorderedListEnd => {}
            Token::SetX(_) => {}
            Token::SV(_) => {}
            Token::SVLemmaBegin(_) => {}
            Token::SVLemmaStop => {}
            Token::CenteredOn => {}
            Token::CenteredOff => {}
            Token::AlignRightOn => {}
            Token::AlignRightOff => {}
            Token::EOn => {}
            Token::EOff => {}
            Token::BibIndex(_) => {}
            Token::NotFirstLine => {}
            Token::Thumb => {}
            Token::EndNew(_) => {}
            Token::UrlBegin(_) => {}
            Token::UrlEnd => {}
            Token::WordAnchor => {}
            Token::ThumbWWW => {}
            Token::S => {}
            Token::NoJustifyOn => {}
            Token::NoJustifyOff => {}
            Token::NextBlankFixed => {}
            Token::WordRest(_) => {}
            Token::WordIncomplete(_) => {}
            Token::HyphenCK => {}
            Token::HebrewOn => {}
            Token::HebrewOff => {}
            Token::NodeNumber2(_) => {}
            Token::StrikeThroughOn => {}
            Token::StrikeThroughOff => {}
            Token::SetY(_) => {}
            Token::Cor(_) => {}
            Token::EndCor => {}
            Token::DashedLine => {}
            Token::Unknown { raw, decoded } => {}
        }
    }

    Ok(())
}

pub fn count_delimeters(s: &str, hyphen: bool) -> usize {
    let v = if !hyphen && !s.chars().all(|c| !c.is_alphanumeric()) {
        1
    } else {
        0
    };

    let mut it = s.chars();

    while let Some(c) = it.next() {
        if c.is_alphanumeric() {
            break;
        }
    }

    while let Some(c) = it.next_back() {
        if c.is_alphanumeric() {
            break;
        }
    }

    v + it.filter(|c| !c.is_alphanumeric()).count()
}
