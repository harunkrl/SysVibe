//! Vitalis — App::command_palette — command-palette input state, extracted
//! from `App`. Owns only the text input + selection index; the command
//! *dispatch* (open/run/execute, which reach into other App groups) stays on
//! `App`.

/// Command-palette input state: the typed query + the highlighted match index.
pub struct CommandPalette {
    input: String,
    selected: usize,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            selected: 0,
        }
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Clear input + reset selection (used when opening the palette).
    pub fn reset(&mut self) {
        self.input.clear();
        self.selected = 0;
    }

    pub fn push(&mut self, c: char) {
        if self.input.chars().count() < 40 {
            self.input.push(c);
            self.selected = 0;
        }
    }

    pub fn backspace(&mut self) {
        self.input.pop();
        self.selected = 0;
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.selected = 0;
    }

    pub fn next(&mut self) {
        self.selected = self.selected.saturating_add(1);
    }

    pub fn prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }
}
