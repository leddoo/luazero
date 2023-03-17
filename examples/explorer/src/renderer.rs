use std::rc::Rc;
use core::cell::RefCell;
use raqote::DrawTarget;
use fontdue::Font;
use lru::LruCache;
use ordered_float::NotNan;


pub struct Renderer {
    target: DrawTarget,
    fonts:  FontCtx,
}

impl Renderer {
    pub fn new(fonts: &FontCtx) -> Renderer {
        Renderer {
            target: DrawTarget::new(1, 1),
            fonts:  fonts.clone(),
        }
    }

    #[allow(dead_code)] // @temp
    #[inline(always)]
    pub fn fonts(&self) -> &FontCtx { &self.fonts }

    #[inline(always)]
    pub fn data(&self) -> &[u32] { self.target.get_data() }

    pub fn set_size(&mut self, w: u32, h: u32) {
        if w as i32 != self.target.width() || h as i32 != self.target.height() {
            self.target = DrawTarget::new(w as i32, h as i32);
        }
    }


    pub fn clear(&mut self, r: u8, g: u8, b: u8) {
        let c = 255 << 24 | (r as u32) << 16 | (g as u32) << 8 | (b as u32);

        let buffer = self.target.get_data_mut();
        let len_8 = buffer.len() & !(8 - 1);

        // this is to make debug builds a little faster.
        // though i should really put this into a separate crate
        // and enable optimizations for dependencies.
        let buffer_base = buffer.as_mut_ptr();
        let mut i = 0;
        while i < len_8 { unsafe {
            buffer_base.add(i + 0).write(c);
            buffer_base.add(i + 1).write(c);
            buffer_base.add(i + 2).write(c);
            buffer_base.add(i + 3).write(c);
            buffer_base.add(i + 4).write(c);
            buffer_base.add(i + 5).write(c);
            buffer_base.add(i + 6).write(c);
            buffer_base.add(i + 7).write(c);
            i += 8;
        }}
        while i < buffer.len() {
            buffer[i] = c;
        }
    }

    pub fn draw_mask_abs(&mut self, x: i32, y: i32, mask: &[u8], mask_w: u32, mask_h: u32, color: u32) {
        let x0 = x;
        let y0 = y;
        let x1 = x0 + mask_w as i32;
        let y1 = y0 + mask_h as i32;

        let tw = self.target.width();
        let th = self.target.height();
        let buf = self.target.get_data_mut();

        let x0c = x0.clamp(0, tw);
        let y0c = y0.clamp(0, th);
        let x1c = x1.clamp(0, tw);
        let y1c = y1.clamp(0, th);


        // taken from swcomposite, cause i ain't adding an explicit dependency for one function.
        // this is an approximation of true 'over' that does a division by 256 instead
        // of 255. It is the same style of blending that Skia does. It corresponds 
        // to Skia's SKPMSrcOver
        #[inline(always)]
        pub fn over(src: u32, dst: u32) -> u32 {
            let a = src >> 24;
            let a = 256 - a;
            let mask = 0xff00ff;
            let rb = ((dst & 0xff00ff) * a) >> 8;
            let ag = ((dst >> 8) & 0xff00ff) * a;
            src + (rb & mask) | (ag & !mask)
        }

        let a = (color >> 24) & 0xff;
        let r = (color >> 16) & 0xff;
        let g = (color >>  8) & 0xff;
        let b = (color >>  0) & 0xff;

        for y in y0c..y1c {
            for x in x0c..x1c {
                let cx = (x - x0) as usize;
                let cy = (y - y0) as usize;
                let c  = mask[cy * mask_w as usize + cx] as u32;

                let a  = a*c >> 8;
                let r  = a*r >> 8;
                let g  = a*g >> 8;
                let b  = a*b >> 8;
                let src = a << 24 | r << 16 | g << 8 | b;

                let value = &mut buf[(y*tw + x) as usize];
                *value = over(src, *value);
            }
        }
    }

    pub fn draw_text_layout_abs(&mut self, x0: i32, y0: i32, layout: &TextLayout<u32>) {
        let fonts = self.fonts.clone();
        let mut fonts = fonts.inner.borrow_mut();

        let mut y0 = y0 as f32;
        for line in &layout.lines {
            let mut x0 = x0 as f32;
            for span in &layout.spans[line.span_range()] {
                for glyph in &layout.glyphs[span.glyph_begin as usize .. span.glyph_end as usize] {
                    let (metrics, mask) = fonts.glyph_mask(span.face_id, span.font_size, glyph.index);
                    self.draw_mask_abs(
                        x0 as i32 + glyph.dx as i32,
                        y0 as i32 + glyph.dy as i32,
                        &mask, metrics.width as u32, metrics.height as u32,
                        span.effect
                    );
                    x0 += glyph.advance;
                }
            }

            y0 += line.height();
        }
    }
}

impl core::ops::Deref for Renderer {
    type Target = DrawTarget;
    #[inline(always)]
    fn deref(&self) -> &Self::Target { &self.target }
}

impl core::ops::DerefMut for Renderer {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.target }
}



#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FaceId(u32);

impl FaceId {
    pub const DEFAULT: FaceId = FaceId(0);

    #[inline(always)]
    pub fn usize(self) -> usize { self.0 as usize }
}

impl Default for FaceId { #[inline(always)] fn default() -> FaceId { FaceId::DEFAULT } }


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bold(pub bool);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Italic(pub bool);


#[derive(Clone)]
pub struct FontCtx {
    inner: Rc<RefCell<FontCtxInner>>,
}

struct FontCtxInner {
    families: Vec<Family>,
    faces: Vec<Face>,

    glyph_cache: LruCache<(FaceId, NotNan<f32>, u16), (fontdue::Metrics, Vec<u8>)>,
}

struct Family {
    name:  String,
    faces: Vec<FaceInfo>,
}

#[derive(Clone, Copy, Debug)]
struct FaceInfo {
    id:     FaceId,
    bold:   Bold,
    italic: Italic,
}

struct Face {
    font: Font,
    #[allow(dead_code)] // @temp
    info: FaceInfo,
}

impl FontCtx {
    pub fn new() -> FontCtx {
        FontCtx {
            inner: Rc::new(RefCell::new(FontCtxInner {
                families: vec![],
                faces:    vec![],

                glyph_cache: LruCache::new(128.try_into().unwrap()),
            })),
        }
    }

    #[allow(dead_code)] // @temp
    pub fn find_face(&self, family: &str, bold: Bold, italic: Italic) -> FaceId {
        let this = self.inner.borrow();

        let family = this.families.iter().find(|f| f.name == family);
        if let Some(family) = family {
            return family.find_face_exact(bold, italic)
                .unwrap_or_else(|| family.faces[0].id);
        }
        FaceId::DEFAULT
    }

    pub fn add_face(&self, family: &str, bold: Bold, italic: Italic, data: &[u8]) -> FaceId {
        let mut this = self.inner.borrow_mut();

        // create face.
        let id = FaceId(this.faces.len() as u32);
        let info = FaceInfo { id, bold, italic };
        let font = Font::from_bytes(data, Default::default()).unwrap();
        this.faces.push(Face { info, font });

        // add to family.
        let fam = this.families.iter_mut().find(|f| f.name == family);
        if let Some(family) = fam {
            assert!(family.find_face_exact(bold, italic).is_none());
            family.faces.push(info);
        }
        else {
            this.families.push(Family { name: family.into(), faces: vec![info] });
        }

        id
    }
}

impl FontCtxInner {
    fn glyph_mask(&mut self, face: FaceId, size: NotNan<f32>, glyph: u16) -> &(fontdue::Metrics, Vec<u8>) {
        self.glyph_cache.get_or_insert((face, size, glyph), || {
            self.faces[face.usize()].font.rasterize_indexed(glyph, *size)
        })
    }
}

impl Family {
    fn find_face_exact(&self, bold: Bold, italic: Italic) -> Option<FaceId> {
        for info in &self.faces {
            if info.bold == bold && info.italic == italic {
                return Some(info.id);
            }
        }
        None
    }
}


pub struct TextLayout<E> {
    fonts:  FontCtx,
    text:   String,
    lines:  Vec<Line>,
    spans:  Vec<Span<E>>,
    glyphs: Vec<Glyph>,
}

#[derive(Clone, Copy, Debug)]
struct Glyph {
    index: u16,
    advance: f32,
    dx: f32,
    dy: f32,
}

struct Line {
    text_begin:  u32,
    text_end:    u32,
    span_begin: u32,
    span_end: u32,

    width:  f32,
    max_ascent:  f32,
    max_descent: f32,
    max_gap:     f32,
}

impl Line {
    #[inline(always)]
    fn span_range(&self) -> core::ops::Range<usize> {
        self.span_begin as usize .. self.span_end as usize
    }
}


struct Span<E> {
    text_begin:  u32,
    text_end:    u32,
    glyph_begin: u32,
    glyph_end:   u32,

    width: f32,

    face_id:   FaceId,
    font_size: NotNan<f32>,
    effect:    E,
}

impl Line {
    #[inline(always)]
    fn new(text_begin: u32, span_begin: u32) -> Line {
        Line {
            text_begin,  text_end:  text_begin,
            span_begin,  span_end:  span_begin,
            width: 0.0,
            max_ascent: 0.0, max_descent: 0.0,
            max_gap: 0.0,
        }
    }

    #[inline(always)]
    fn height(&self) -> f32 {
        self.max_ascent + self.max_descent + self.max_gap
    }
}

impl<E> TextLayout<E> {
    pub fn new(fonts: &FontCtx) -> Self {
        TextLayout {
            fonts:  fonts.clone(),
            text:   "".into(),
            lines:  vec![Line::new(0, 0)],
            spans:  vec![],
            glyphs: vec![],
        }
    }

    #[inline(always)]
    pub fn text(&self) -> &str { &self.text }

    pub fn clear(&mut self) {
        self.text.clear();
        self.lines.clear();
        self.spans.clear();
        self.glyphs.clear();
        self.lines.push(Line::new(0, 0));
    }

    pub fn append_ex(&mut self, text: &str, face_id: FaceId, font_size: f32, effect: E) where E: Copy {
        let font_size = NotNan::new(font_size).unwrap();

        let mut current_line = self.lines.last_mut().unwrap();
        let mut pos_cursor   = current_line.width;

        let mut text_cursor = self.text.len() as u32;
        self.text.push_str(text);

        let fonts = self.fonts.clone();
        let fonts = fonts.inner.borrow();
        let face  = &fonts.faces[face_id.usize()];

        let font_metrics = face.font.horizontal_line_metrics(*font_size).unwrap();
        current_line.max_ascent  = current_line.max_ascent.max(font_metrics.ascent);
        current_line.max_descent = current_line.max_descent.max(-font_metrics.descent);
        current_line.max_gap     = current_line.max_gap.max(font_metrics.line_gap);

        let mut text = text;
        while text.len() > 0 {
            let (segment_end, is_line) =
                text.find('\n').map(|index| (index, true))
                .unwrap_or((text.len(), false));

            let pos_begin = pos_cursor;

            // add glyphs.
            // assume one glyph per char.
            let glyph_begin = self.glyphs.len() as u32;
            for c in text[..segment_end].chars() {
                // @todo: kern.
                let glyph_index = face.font.lookup_glyph_index(c);
                let metrics     = face.font.metrics_indexed(glyph_index, *font_size);

                self.glyphs.push(Glyph {
                    index: glyph_index,
                    advance: metrics.advance_width,
                    dx: metrics.bounds.xmin,
                    dy: -metrics.bounds.height - metrics.bounds.ymin,
                });

                pos_cursor += metrics.advance_width;
            }
            assert_eq!(self.glyphs.len(), glyph_begin as usize + segment_end);
            let glyph_end = self.glyphs.len() as u32;

            // add span.
            self.spans.push(Span {
                text_begin: text_cursor,
                text_end:   text_cursor + segment_end as u32,
                glyph_begin, glyph_end,
                width: pos_cursor - pos_begin,
                face_id, font_size, effect,
            });


            // update line.
            let line_end = text_cursor + segment_end as u32;
            current_line.text_end  = line_end;
            current_line.span_end += 1;
            current_line.width     = pos_cursor;

            if is_line {
                self.new_line();
                current_line = self.lines.last_mut().unwrap();
                pos_cursor   = 0.0;

                // need to fix up text begin/end,
                // as we've added the entire `text` already.
                // line ranges are weird. they don't include the `\n`.
                // so the next line starts after the current line's `\n`.
                current_line.text_begin  = line_end + 1;
                current_line.text_end    = line_end + 1;

                current_line.max_ascent  = font_metrics.ascent;
                current_line.max_descent = -font_metrics.descent;
                current_line.max_gap     = font_metrics.line_gap;
            }

            let text_advance = segment_end + is_line as usize;
            text_cursor += text_advance as u32;
            text = &text[text_advance..];
        }
    }

    pub fn new_line(&mut self) {
        let text_begin  = self.text.len()   as u32;
        let span_begin  = self.spans.len()  as u32;
        self.lines.push(Line::new(text_begin, span_begin));
    }
}


#[derive(Clone, Copy, Debug)]
pub struct PosMetrics {
    pub x: f32,
    pub y: f32,
    pub glyph_width: f32,
    pub line_height: f32,
    pub line_index:  usize,
}

impl<E> TextLayout<E> {
    pub fn hit_test_text_pos(&self, pos: usize) -> PosMetrics {
        let pos = pos.min(self.text.len()) as u32;

        let mut y = 0.0;
        for (line_index, line) in self.lines.iter().enumerate() {
            let mut x = 0.0;
            if pos >= line.text_begin && pos <= line.text_end {
                for span in &self.spans[line.span_range()] {
                    // in current span.
                    if pos >= span.text_begin
                    && pos <  span.text_end {
                        let offset = (pos - span.text_begin) as usize;
                        let glyph_begin = span.glyph_begin as usize;

                        for i in 0..offset {
                            x += self.glyphs[glyph_begin + i].advance;
                        }

                        let advance = self.glyphs[glyph_begin + offset].advance;
                        return PosMetrics {
                            x, y,
                            glyph_width: advance,
                            line_height: line.height(),
                            line_index,
                        };
                    }

                    x += span.width;
                }

                // end of line.
                let x = line.width;
                return PosMetrics {
                    x, y,
                    glyph_width: 1.,
                    line_height: line.height(),
                    line_index,
                };
            }

            y += line.height();
        }

        assert_eq!(self.text.len(), 0);
        return PosMetrics { x: 0.0, y: 0.0, glyph_width: 0.0, line_height: 0.0, line_index: 0 };
    }
}
