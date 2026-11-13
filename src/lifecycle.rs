/// Generation-tagged transitions keep late worker results from reopening a dismissed viewer.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Idle,
    Capturing(u64),
    Visible,
}

#[derive(Debug, Default)]
pub struct Lifecycle {
    state: State,
    generation: u64,
}
impl Lifecycle {
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn begin(&mut self) -> Option<u64> {
        if self.state != State::Idle {
            return None;
        }
        self.generation = self.generation.checked_add(1)?;
        self.state = State::Capturing(self.generation);
        Some(self.generation)
    }
    pub fn accepts(&self, generation: u64) -> bool {
        self.state == State::Capturing(generation)
    }
    pub fn show(&mut self, generation: u64) -> bool {
        if !self.accepts(generation) {
            return false;
        }
        self.state = State::Visible;
        true
    }
    pub fn dismiss(&mut self) {
        self.state = State::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_and_duplicate_workers_cannot_reopen_viewer() {
        let mut life = Lifecycle::default();
        let old = life.begin().unwrap();
        assert!(life.begin().is_none());
        life.dismiss();
        let new = life.begin().unwrap();
        assert!(!life.show(old));
        assert!(life.show(new));
        assert!(!life.show(new));
        life.dismiss();
        assert!(!life.show(new));
    }
}
