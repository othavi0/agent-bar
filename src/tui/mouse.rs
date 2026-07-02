//! Hit-testing de mouse: o render registra regiões clicáveis; o event_loop
//! consulta no MouseEvent. update() permanece puro.

use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipKind {
    Open,
    Refresh,
    Help,
    Quit,
    Back,
    Login,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTarget {
    Sidebar(usize),
    Card(usize),
    Chip(ChipKind),
}

#[derive(Debug, Default)]
pub struct HitMap {
    zones: Vec<(Rect, MouseTarget)>,
}

impl HitMap {
    pub fn clear(&mut self) {
        self.zones.clear();
    }

    pub fn push(&mut self, rect: Rect, t: MouseTarget) {
        self.zones.push((rect, t));
    }

    pub fn at(&self, x: u16, y: u16) -> Option<MouseTarget> {
        self.zones
            .iter()
            .rev()
            .find(|(r, _)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
            .map(|(_, t)| *t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn hitmap_last_registered_wins() {
        let mut h = HitMap::default();
        h.push(Rect::new(0, 0, 10, 10), MouseTarget::Card(0));
        h.push(Rect::new(2, 2, 3, 3), MouseTarget::Chip(ChipKind::Refresh));
        assert_eq!(h.at(3, 3), Some(MouseTarget::Chip(ChipKind::Refresh)));
        assert_eq!(h.at(0, 0), Some(MouseTarget::Card(0)));
        assert_eq!(h.at(50, 50), None);
        h.clear();
        assert_eq!(h.at(3, 3), None);
    }
}
