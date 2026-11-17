// High-performance byte-aware undo/redo manager for editor state
use std::time::Instant;

const MAX_HISTORY: usize = 120;
const DEBOUNCE_MILLIS: u128 = 450;

#[derive(Clone, Debug)]
pub struct TextSnapshot {
    pub text: String,
    pub cursor_offset: i32,
}

pub struct UndoManager {
    undo_stack: Vec<TextSnapshot>,
    redo_stack: Vec<TextSnapshot>,
    current_snapshot: TextSnapshot,
    last_record_time: Instant,
}

impl UndoManager {
    pub fn new(initial_text: String, initial_cursor: i32) -> Self {
        Self {
            undo_stack: Vec::with_capacity(64),
            redo_stack: Vec::with_capacity(32),
            current_snapshot: TextSnapshot {
                text: initial_text,
                cursor_offset: initial_cursor,
            },
            last_record_time: Instant::now(),
        }
    }

    pub fn reset(&mut self, text: String, cursor: i32) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current_snapshot = TextSnapshot {
            text,
            cursor_offset: cursor,
        };
        self.last_record_time = Instant::now();
    }

    pub fn record_edit(&mut self, new_text: &str, cursor: i32, force_snapshot: bool) {
        if new_text == self.current_snapshot.text {
            self.current_snapshot.cursor_offset = cursor;
            return;
        }

        let elapsed = self.last_record_time.elapsed().as_millis();
        let should_push = force_snapshot
            || elapsed > DEBOUNCE_MILLIS
            || self.should_break_undo_group(&self.current_snapshot.text, new_text);

        if should_push {
            if self.undo_stack.len() >= MAX_HISTORY {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(self.current_snapshot.clone());
            self.redo_stack.clear();
            self.last_record_time = Instant::now();
        }

        self.current_snapshot = TextSnapshot {
            text: new_text.to_string(),
            cursor_offset: cursor,
        };
    }

    pub fn undo(&mut self) -> Option<TextSnapshot> {
        let previous = self.undo_stack.pop()?;
        self.redo_stack.push(self.current_snapshot.clone());
        self.current_snapshot = previous.clone();
        self.last_record_time = Instant::now();
        Some(previous)
    }

    pub fn redo(&mut self) -> Option<TextSnapshot> {
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(self.current_snapshot.clone());
        self.current_snapshot = next.clone();
        self.last_record_time = Instant::now();
        Some(next)
    }

    fn should_break_undo_group(&self, old_text: &str, new_text: &str) -> bool {
        let old_b = old_text.as_bytes();
        let new_b = new_text.as_bytes();
        let diff = (new_b.len() as isize) - (old_b.len() as isize);
        if diff.abs() > 4 {
            return true;
        }
        if new_b.ends_with(b"\n") || new_b.ends_with(b" ") {
            return true;
        }
        false
    }
}
