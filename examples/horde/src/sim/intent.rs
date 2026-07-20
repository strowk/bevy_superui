use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    Move(Vec2),
    Aim(Vec2),
    Shoot(bool),
    SwitchWeapon(usize),
    CycleWeapon(i32),
    ToggleInventory,
    Pause,
    Resume,
    Restart,
    StartGame,
    Quit,
}

#[derive(Resource, Default)]
pub struct IntentQueue(pub Vec<Intent>);

impl IntentQueue {
    pub fn push(&mut self, i: Intent) {
        self.0.push(i);
    }
    pub fn drain(&mut self) -> std::vec::Drain<'_, Intent> {
        self.0.drain(..)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_empties_queue() {
        let mut q = IntentQueue::default();
        q.push(Intent::Pause);
        q.push(Intent::Resume);
        let drained: Vec<_> = q.drain().collect();
        assert_eq!(drained, vec![Intent::Pause, Intent::Resume]);
        assert!(q.0.is_empty());
    }
}
