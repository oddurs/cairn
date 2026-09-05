// cairn — column-aligned output.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.
use terminal_size::{Width, terminal_size};
use unicode_width::UnicodeWidthStr;

pub struct Cell {
    text: String,
    styled: Option<String>,
}

impl Cell {
    pub fn plain(text: impl Into<String>) -> Cell {
        Cell {
            text: text.into(),
            styled: None,
        }
    }

    pub fn styled(text: impl Into<String>, styled: impl Into<String>) -> Cell {
        let text = text.into();
        // An empty cell has nothing to colour; wrapping it in escape codes just
        // emits noise into the output.
        let styled = if text.is_empty() {
            None
        } else {
            Some(styled.into())
        };
        Cell { text, styled }
    }
}

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<Cell>>,
}

impl Table {
    pub fn new(headers: &[&str]) -> Table {
        Table {
            headers: headers.iter().map(|h| h.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn row(&mut self, cells: Vec<Cell>) {
        self.rows.push(cells);
    }

    pub fn render(&self) -> String {
        let ncols = self.headers.len();
        if ncols == 0 {
            return String::new();
        }
        let gap = 2;
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.width()).collect();
        for row in &self.rows {
            for (i, c) in row.iter().enumerate().take(ncols) {
                widths[i] = widths[i].max(c.text.width());
            }
        }

        // The last column absorbs whatever room is left over.
        let total: usize = widths.iter().sum::<usize>() + gap * (ncols - 1);
        let term = terminal_size()
            .map(|(Width(w), _)| w as usize)
            .unwrap_or(100);
        if total > term {
            let others: usize = widths[..ncols - 1].iter().sum::<usize>() + gap * (ncols - 1);
            widths[ncols - 1] = term.saturating_sub(others).max(12);
        }

        let sep = " ".repeat(gap);
        let mut out = String::new();
        let header: Vec<String> = self
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| pad(&h.to_uppercase(), widths[i], i == ncols - 1))
            .collect();
        out.push_str(crate::style::dim(header.join(&sep).trim_end()).as_str());
        out.push('\n');

        for row in &self.rows {
            let mut cells = Vec::with_capacity(ncols);
            for (i, width) in widths.iter().enumerate() {
                let (text, styled) = match row.get(i) {
                    Some(c) => (c.text.as_str(), c.styled.as_deref()),
                    None => ("", None),
                };
                let last = i == ncols - 1;
                if text.width() > *width {
                    // Clipping discards the styled form; only the free-form last
                    // column is ever wide enough to hit this.
                    cells.push(pad(&clip(text, *width), *width, last));
                } else if last {
                    cells.push(styled.unwrap_or(text).to_string());
                } else {
                    let padding = " ".repeat(width.saturating_sub(text.width()));
                    cells.push(format!("{}{padding}", styled.unwrap_or(text)));
                }
            }
            out.push_str(cells.join(&sep).trim_end());
            out.push('\n');
        }
        out
    }
}

fn pad(s: &str, width: usize, last: bool) -> String {
    if last {
        return s.to_string();
    }
    let w = s.width();
    if w >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - w))
    }
}

pub fn clip(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = c.to_string().width();
        if w + cw > max.saturating_sub(1) {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}
