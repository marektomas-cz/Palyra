use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

const WORD_SEPARATORS: &[char] = &[
    ' ', '\t', '\n', '\r', '/', '\\', ':', ';', ',', '.', '!', '?', '(', ')', '[', ']', '{', '}',
    '<', '>', '-', '_', '"', '\'', '`',
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TuiComposer {
    text: String,
    cursor: usize,
    selection_anchor: Option<usize>,
    preferred_column: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiComposerView {
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) cursor_x: u16,
    pub(crate) cursor_y: u16,
    pub(crate) total_lines: usize,
}

#[derive(Debug, Clone, Copy)]
struct LineInfo {
    start: usize,
    end: usize,
}

impl TuiComposer {
    pub(crate) fn text(&self) -> &str {
        self.text.as_str()
    }

    pub(crate) fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len();
        self.selection_anchor = None;
        self.preferred_column = None;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.selection_anchor = None;
        self.preferred_column = None;
    }

    pub(crate) fn trimmed_text(&self) -> &str {
        self.text.trim()
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.selected_range().is_some()
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub(crate) fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.text.len();
        self.preferred_column = None;
    }

    pub(crate) fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let insertion_point = if let Some(range) = self.take_selected_range() {
            self.text.replace_range(range.clone(), text);
            range.start
        } else {
            self.text.insert_str(self.cursor, text);
            self.cursor
        };
        self.cursor = insertion_point + text.len();
        self.preferred_column = None;
    }

    pub(crate) fn insert_newline(&mut self) {
        self.insert_text("\n");
    }

    pub(crate) fn backspace(&mut self) {
        if let Some(range) = self.take_selected_range() {
            self.text.replace_range(range.clone(), "");
            self.cursor = range.start;
            self.preferred_column = None;
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let previous = prev_char_boundary(self.text.as_str(), self.cursor);
        self.text.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.preferred_column = None;
    }

    pub(crate) fn delete(&mut self) {
        if let Some(range) = self.take_selected_range() {
            self.text.replace_range(range.clone(), "");
            self.cursor = range.start;
            self.preferred_column = None;
            return;
        }
        if self.cursor >= self.text.len() {
            return;
        }
        let next = next_char_boundary(self.text.as_str(), self.cursor);
        self.text.replace_range(self.cursor..next, "");
        self.preferred_column = None;
    }

    pub(crate) fn move_left(&mut self, selecting: bool, by_word: bool) {
        if !selecting {
            if let Some(range) = self.take_selected_range() {
                self.cursor = range.start;
                self.preferred_column = None;
                return;
            }
        }
        let target = if by_word {
            prev_word_boundary(self.text.as_str(), self.cursor)
        } else {
            prev_char_boundary(self.text.as_str(), self.cursor)
        };
        self.move_cursor(target, selecting);
    }

    pub(crate) fn move_right(&mut self, selecting: bool, by_word: bool) {
        if !selecting {
            if let Some(range) = self.take_selected_range() {
                self.cursor = range.end;
                self.preferred_column = None;
                return;
            }
        }
        let target = if by_word {
            next_word_boundary(self.text.as_str(), self.cursor)
        } else {
            next_char_boundary(self.text.as_str(), self.cursor)
        };
        self.move_cursor(target, selecting);
    }

    pub(crate) fn move_to_line_start(&mut self, selecting: bool) {
        let (line_index, _) = self.cursor_line_col();
        let lines = line_infos(self.text.as_str());
        let target = lines.get(line_index).map(|line| line.start).unwrap_or_default();
        self.move_cursor(target, selecting);
    }

    pub(crate) fn move_to_line_end(&mut self, selecting: bool) {
        let (line_index, _) = self.cursor_line_col();
        let lines = line_infos(self.text.as_str());
        let target = lines.get(line_index).map(|line| line.end).unwrap_or(self.text.len());
        self.move_cursor(target, selecting);
    }

    pub(crate) fn move_to_start(&mut self, selecting: bool) {
        self.move_cursor(0, selecting);
    }

    pub(crate) fn move_to_end(&mut self, selecting: bool) {
        self.move_cursor(self.text.len(), selecting);
    }

    pub(crate) fn move_vertical(&mut self, delta: isize, selecting: bool) {
        let lines = line_infos(self.text.as_str());
        if lines.is_empty() {
            self.move_cursor(0, selecting);
            return;
        }
        let (line_index, column) = self.cursor_line_col();
        let current_line = line_index as isize;
        let target_line = (current_line + delta).clamp(0, lines.len() as isize - 1) as usize;
        let desired_column = self.preferred_column.unwrap_or(column);
        let target = byte_index_for_line_column(self.text.as_str(), target_line, desired_column);
        self.move_cursor(target, selecting);
        self.preferred_column = Some(desired_column);
    }

    pub(crate) fn render(&self, max_visible_lines: usize, focused: bool) -> TuiComposerView {
        let lines = line_infos(self.text.as_str());
        let total_lines = lines.len().max(1);
        let max_visible_lines = max_visible_lines.max(1);
        let (cursor_line, cursor_col) = self.cursor_line_col();
        let start_line = cursor_line.saturating_add(1).saturating_sub(max_visible_lines);
        let end_line = (start_line + max_visible_lines).min(total_lines);
        let selected = self.selected_range();
        let selection_style = if focused {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().add_modifier(Modifier::UNDERLINED)
        };
        let rendered_lines = (start_line..end_line)
            .map(|index| {
                let info = lines.get(index).copied().unwrap_or(LineInfo { start: 0, end: 0 });
                render_line(self.text.as_str(), info, selected.as_ref(), selection_style)
            })
            .collect::<Vec<_>>();
        TuiComposerView {
            lines: if rendered_lines.is_empty() { vec![Line::default()] } else { rendered_lines },
            cursor_x: cursor_col as u16,
            cursor_y: cursor_line.saturating_sub(start_line) as u16,
            total_lines,
        }
    }

    pub(crate) fn cursor_line_col(&self) -> (usize, usize) {
        cursor_line_col(self.text.as_str(), self.cursor)
    }

    pub(crate) fn selected_range(&self) -> Option<std::ops::Range<usize>> {
        let anchor = self.selection_anchor?;
        if anchor == self.cursor {
            return None;
        }
        if anchor < self.cursor {
            Some(anchor..self.cursor)
        } else {
            Some(self.cursor..anchor)
        }
    }

    fn take_selected_range(&mut self) -> Option<std::ops::Range<usize>> {
        let range = self.selected_range();
        self.selection_anchor = None;
        range
    }

    fn move_cursor(&mut self, next: usize, selecting: bool) {
        let next = next.min(self.text.len());
        if selecting {
            self.selection_anchor.get_or_insert(self.cursor);
        } else {
            self.selection_anchor = None;
        }
        self.cursor = next;
        if !selecting {
            self.preferred_column = None;
        }
    }
}

fn render_line(
    text: &str,
    info: LineInfo,
    selected: Option<&std::ops::Range<usize>>,
    selection_style: Style,
) -> Line<'static> {
    let line_text = &text[info.start..info.end];
    let Some(range) = selected else {
        return Line::from(line_text.to_owned());
    };
    if range.end <= info.start || range.start >= info.end {
        return Line::from(line_text.to_owned());
    }
    let highlight_start = range.start.max(info.start);
    let highlight_end = range.end.min(info.end);
    let mut spans = Vec::new();
    if highlight_start > info.start {
        spans.push(Span::raw(text[info.start..highlight_start].to_owned()));
    }
    spans.push(Span::styled(text[highlight_start..highlight_end].to_owned(), selection_style));
    if highlight_end < info.end {
        spans.push(Span::raw(text[highlight_end..info.end].to_owned()));
    }
    Line::from(spans)
}

fn cursor_line_col(text: &str, cursor: usize) -> (usize, usize) {
    let lines = line_infos(text);
    for (index, line) in lines.iter().enumerate() {
        if cursor <= line.end {
            return (index, text[line.start..cursor].chars().count());
        }
        if index + 1 == lines.len() {
            return (index, text[line.start..line.end].chars().count());
        }
    }
    (0, 0)
}

fn byte_index_for_line_column(text: &str, line_index: usize, column: usize) -> usize {
    let lines = line_infos(text);
    let Some(line) = lines.get(line_index).copied() else {
        return text.len();
    };
    let mut current_column = 0usize;
    for (offset, ch) in text[line.start..line.end].char_indices() {
        if current_column == column {
            return line.start + offset;
        }
        current_column += 1;
        if current_column > column {
            return line.start + offset + ch.len_utf8();
        }
    }
    line.end
}

fn prev_char_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor].char_indices().last().map(|(index, _)| index).unwrap_or(0)
}

fn next_char_boundary(text: &str, cursor: usize) -> usize {
    if cursor >= text.len() {
        return text.len();
    }
    let slice = &text[cursor..];
    if let Some(ch) = slice.chars().next() {
        cursor + ch.len_utf8()
    } else {
        text.len()
    }
}

fn prev_word_boundary(text: &str, cursor: usize) -> usize {
    let mut boundary = 0usize;
    let chars = text[..cursor].char_indices().collect::<Vec<_>>();
    let mut index = chars.len();
    while index > 0 {
        index -= 1;
        let (byte_index, ch) = chars[index];
        if !WORD_SEPARATORS.contains(&ch) {
            boundary = byte_index;
            break;
        }
    }
    while boundary > 0 {
        let previous = prev_char_boundary(text, boundary);
        let ch = text[previous..boundary].chars().next().unwrap_or_default();
        if WORD_SEPARATORS.contains(&ch) {
            break;
        }
        boundary = previous;
    }
    boundary
}

fn next_word_boundary(text: &str, cursor: usize) -> usize {
    let mut index = cursor;
    while index < text.len() {
        let next = next_char_boundary(text, index);
        let ch = text[index..next].chars().next().unwrap_or_default();
        if !WORD_SEPARATORS.contains(&ch) {
            break;
        }
        index = next;
    }
    while index < text.len() {
        let next = next_char_boundary(text, index);
        let ch = text[index..next].chars().next().unwrap_or_default();
        if WORD_SEPARATORS.contains(&ch) {
            break;
        }
        index = next;
    }
    index
}

fn line_infos(text: &str) -> Vec<LineInfo> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            lines.push(LineInfo { start, end: index });
            start = index + ch.len_utf8();
        }
    }
    lines.push(LineInfo { start, end: text.len() });
    lines
}

#[cfg(test)]
mod tests {
    use super::TuiComposer;

    #[test]
    fn multiline_insert_and_vertical_navigation_preserve_column() {
        let mut composer = TuiComposer::default();
        composer.insert_text("alpha\nbeta\ngamma");
        composer.move_to_start(false);
        composer.move_right(false, false);
        composer.move_right(false, false);
        composer.move_vertical(1, false);
        assert_eq!(composer.cursor_line_col(), (1, 2));
        composer.move_vertical(1, false);
        assert_eq!(composer.cursor_line_col(), (2, 2));
    }

    #[test]
    fn paste_replaces_selected_text() {
        let mut composer = TuiComposer::default();
        composer.insert_text("alpha beta");
        composer.move_to_start(false);
        composer.move_right(true, true);
        composer.insert_text("merged");
        assert_eq!(composer.text(), "merged beta");
        assert!(!composer.has_selection());
    }

    #[test]
    fn select_all_then_backspace_clears_editor() {
        let mut composer = TuiComposer::default();
        composer.insert_text("payload");
        composer.select_all();
        composer.backspace();
        assert!(composer.text().is_empty());
        assert_eq!(composer.cursor_line_col(), (0, 0));
    }

    #[test]
    fn render_scrolls_to_last_visible_lines() {
        let mut composer = TuiComposer::default();
        composer.insert_text("one\ntwo\nthree\nfour");
        let view = composer.render(2, true);
        assert_eq!(view.total_lines, 4);
        assert_eq!(view.lines.len(), 2);
        assert_eq!(view.cursor_y, 1);
    }
}
