use std::num::{NonZeroU16, NonZeroU8};

use crate::{
    encoder::{self, Encoder},
    for_flutter_proto,
};

#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
struct ChunkStyle {
    emphasis: bool,
    strong: bool,
    superscript: bool,
    subscript: bool,
    strikethrough: bool,
    underline: bool,
    wide_spacing: bool,
    size: Option<NonZeroU8>,
    colour_gray: bool,
}

impl ChunkStyle {
    fn to_proto(self) -> for_flutter_proto::ChunkStyle {
        let Self {
            emphasis,
            strong,
            superscript,
            subscript,
            strikethrough,
            underline,
            wide_spacing,
            size,
            colour_gray,
        } = self;

        for_flutter_proto::ChunkStyle {
            emphasis,
            strong,
            superscript,
            subscript,
            strikethrough,
            underline,
            wide_spacing,
            colour_gray,
            size: size.map_or(1.0, |x| u8::from(x) as f32 / 100.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
struct SegmentStyle {
    left_padding: Option<NonZeroU16>,
    no_justification: bool,
    alignment: Option<String>,
}

impl SegmentStyle {
    fn to_proto(self) -> for_flutter_proto::SegmentStyle {
        let Self {
            left_padding,
            no_justification,
            alignment,
        } = self;

        let alignment = match alignment.as_deref() {
            Some("center") => for_flutter_proto::Alignment::Center,
            Some("right") => for_flutter_proto::Alignment::Right,
            _ => {
                if no_justification {
                    for_flutter_proto::Alignment::Unjustified
                } else {
                    for_flutter_proto::Alignment::Justified
                }
            }
        };

        for_flutter_proto::SegmentStyle {
            left_padding: left_padding.map_or(0.0, |x| u16::from(x) as f32 / 100.0),
            alignment: alignment.into(),
        }
    }
}

fn split_style(s: encoder::Style) -> (ChunkStyle, SegmentStyle) {
    let encoder::Style {
        left_padding,
        emphasis,
        strong,
        superscript,
        subscript,
        strikethrough,
        underline,
        wide_spacing,
        size,
        color_gray,
        no_justification,
        alignment,
    } = s;

    let c = ChunkStyle {
        emphasis,
        strong,
        superscript,
        subscript,
        strikethrough,
        underline,
        wide_spacing,
        size,
        colour_gray: color_gray,
    };

    let s = SegmentStyle {
        left_padding,
        no_justification,
        alignment: alignment.map(|s| s.to_owned()),
    };

    (c, s)
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
enum Piece {
    Chunk { style: ChunkStyle, text: String },
    Link { url: String, content: String },
    PageRef(u32),
    SearchWord(String),
}

impl Piece {
    fn to_proto(self) -> for_flutter_proto::Piece {
        let body = match self {
            Piece::Chunk { style, text } => {
                for_flutter_proto::piece::Body::Chunk(for_flutter_proto::Chunk { style: Some(style.to_proto()),
                text }
                )
            },
            Piece::Link { url, content: text } => {
                for_flutter_proto::piece::Body::Link(for_flutter_proto::Link { url, text})
            },
            Piece::PageRef(number) => {
                for_flutter_proto::piece::Body::PageRef(for_flutter_proto::PageRef { r#ref: number })
            },
            Piece::SearchWord(word) => {
                for_flutter_proto::piece::Body::SearchWord(for_flutter_proto::SearchWord { word
                 })
            },
        };

        for_flutter_proto::Piece { body: Some(body) }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Segment {
    style: SegmentStyle,
    pieces: Vec<Piece>,
}

impl Segment {
    fn to_proto(self) -> for_flutter_proto::Segment {
        for_flutter_proto::Segment { style: Some(self.style.to_proto()), pieces: self.pieces.into_iter().map(|p| p.to_proto()).collect() }
    }
}

impl Segment {
    fn new() -> Self {
        Self {
            style: Default::default(),
            pieces: Vec::new(),
        }
    }

    fn new_with_piece(style: SegmentStyle, piece: Piece) -> Self {
        Self {
            style,
            pieces: vec![piece],
        }
    }

    fn push_piece(&mut self, piece: Piece) {
        if let Piece::Chunk {
            style: new_style,
            text: new_text,
        } = &piece
        {
            if let Some(Piece::Chunk { style, text }) = self.pieces.last_mut() {
                if style == new_style {
                    text.push_str(&new_text);
                    return;
                }
            }
        }

        self.pieces.push(piece);
    }
}

pub struct ForFlutter {
    pub plain: String,
    segments: Vec<Segment>,
}

impl ForFlutter {
    pub fn new() -> Self {
        Self {
            plain: String::new(),
            segments: vec![Segment::new()],
        }
    }

    pub fn to_proto(self) -> for_flutter_proto::Segments {
        for_flutter_proto::Segments { segments: self.segments.into_iter().map(|s| s.to_proto()).collect() }
    }

    fn push_piece_samestyle(&mut self, piece: Piece) {
        self.segments.last_mut().unwrap().push_piece(piece);
    }

    fn push_piece(&mut self, style: SegmentStyle, piece: Piece) {
        if self.segments.last().unwrap().style == style {
            self.segments.last_mut().unwrap().push_piece(piece);
        } else {
            self.segments.push(Segment::new_with_piece(style, piece));
        }
    }
}

impl Encoder for ForFlutter {
    fn chunk(&mut self, s: &str, style: &crate::encoder::Style) {
        self.plain.push_str(s);
        let (chunk_style, segment_style) = split_style(style.clone());
        self.push_piece(
            segment_style,
            Piece::Chunk {
                style: chunk_style,
                text: s.to_owned(),
            },
        );
    }

    fn link(&mut self, url: &str, content: &str) {
        self.push_piece_samestyle(Piece::Link {
            url: url.to_owned(),
            content: content.to_owned(),
        });
    }

    fn pageref(&mut self, page: u32) {
        self.push_piece_samestyle(Piece::PageRef(page));
    }

    fn searchword(&mut self, s: &str) {
        self.push_piece_samestyle(Piece::SearchWord(s.to_owned()));
    }
}
