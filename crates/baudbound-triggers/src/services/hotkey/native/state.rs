use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct NativeChord {
    pub(super) modifiers: u8,
    pub(super) virtual_keys: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyTransition {
    Down,
    Up,
}

pub(super) struct KeyboardState {
    bindings: BTreeMap<NativeChord, String>,
    modifier_keys: BTreeMap<u32, u8>,
    pressed_keys: BTreeSet<u32>,
    pressed_modifiers: BTreeSet<u32>,
}

impl KeyboardState {
    pub(super) fn new(
        bindings: BTreeMap<NativeChord, String>,
        modifier_keys: BTreeMap<u32, u8>,
    ) -> Self {
        Self {
            bindings,
            modifier_keys,
            pressed_keys: BTreeSet::new(),
            pressed_modifiers: BTreeSet::new(),
        }
    }

    pub(super) fn reconfigure(&mut self, bindings: BTreeMap<NativeChord, String>) {
        self.bindings = bindings;
    }

    pub(super) fn process(
        &mut self,
        virtual_key: u32,
        transition: KeyTransition,
        injected: bool,
    ) -> Option<String> {
        if injected {
            return None;
        }

        if self.modifier_keys.contains_key(&virtual_key) {
            match transition {
                KeyTransition::Down => {
                    let previous_modifiers = self.current_modifiers();
                    if !self.pressed_modifiers.insert(virtual_key)
                        || previous_modifiers == self.current_modifiers()
                    {
                        return None;
                    }
                    return self.match_pressed_chord();
                }
                KeyTransition::Up => {
                    self.pressed_modifiers.remove(&virtual_key);
                }
            }
            return None;
        }

        match transition {
            KeyTransition::Down => {
                if !self.pressed_keys.insert(virtual_key) {
                    return None;
                }
                self.match_pressed_chord()
            }
            KeyTransition::Up => {
                self.pressed_keys.remove(&virtual_key);
                None
            }
        }
    }

    fn current_modifiers(&self) -> u8 {
        self.pressed_modifiers
            .iter()
            .filter_map(|virtual_key| self.modifier_keys.get(virtual_key))
            .fold(0, |state, modifier| state | modifier)
    }

    fn match_pressed_chord(&self) -> Option<String> {
        let chord = NativeChord {
            modifiers: self.current_modifiers(),
            virtual_keys: self.pressed_keys.iter().copied().collect(),
        };
        self.bindings.get(&chord).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL_LEFT: u32 = 162;
    const CTRL_RIGHT: u32 = 163;
    const KEY_A: u32 = 65;
    const KEY_B: u32 = 66;

    fn state(bindings: impl IntoIterator<Item = (u32, u8, &'static str)>) -> KeyboardState {
        KeyboardState::new(
            bindings
                .into_iter()
                .map(|(virtual_key, modifiers, expression)| {
                    (
                        NativeChord {
                            modifiers,
                            virtual_keys: vec![virtual_key],
                        },
                        expression.to_owned(),
                    )
                })
                .collect(),
            BTreeMap::from([(CTRL_LEFT, 1), (CTRL_RIGHT, 1)]),
        )
    }

    #[test]
    fn matches_unmodified_keys_and_suppresses_repeat_until_key_up() {
        let mut state = state([(KEY_A, 0, "A")]);

        assert_eq!(
            state.process(KEY_A, KeyTransition::Down, false),
            Some("A".to_owned())
        );
        assert_eq!(state.process(KEY_A, KeyTransition::Down, false), None);
        assert_eq!(state.process(KEY_A, KeyTransition::Up, false), None);
        assert_eq!(
            state.process(KEY_A, KeyTransition::Down, false),
            Some("A".to_owned())
        );
    }

    #[test]
    fn requires_exact_modifier_state_and_tracks_both_modifier_sides() {
        let mut state = state([(KEY_A, 0, "A"), (KEY_A, 1, "Ctrl+A")]);

        state.process(CTRL_LEFT, KeyTransition::Down, false);
        state.process(CTRL_RIGHT, KeyTransition::Down, false);
        assert_eq!(
            state.process(KEY_A, KeyTransition::Down, false),
            Some("Ctrl+A".to_owned())
        );
        state.process(KEY_A, KeyTransition::Up, false);
        state.process(CTRL_LEFT, KeyTransition::Up, false);
        assert_eq!(
            state.process(KEY_A, KeyTransition::Down, false),
            Some("Ctrl+A".to_owned())
        );
        state.process(KEY_A, KeyTransition::Up, false);
        state.process(CTRL_RIGHT, KeyTransition::Up, false);
        assert_eq!(
            state.process(KEY_A, KeyTransition::Down, false),
            Some("A".to_owned())
        );
    }

    #[test]
    fn ignores_injected_events_without_changing_physical_key_state() {
        let mut state = state([(KEY_A, 0, "A")]);

        assert_eq!(state.process(KEY_A, KeyTransition::Down, true), None);
        assert_eq!(
            state.process(KEY_A, KeyTransition::Down, false),
            Some("A".to_owned())
        );
        assert_eq!(state.process(KEY_A, KeyTransition::Up, true), None);
        assert_eq!(state.process(KEY_A, KeyTransition::Down, false), None);
    }

    #[test]
    fn reconfigure_replaces_bindings_without_resetting_pressed_state() {
        let mut state = state([(KEY_A, 0, "A")]);
        state.process(KEY_A, KeyTransition::Down, false);
        state.reconfigure(BTreeMap::from([(
            NativeChord {
                modifiers: 0,
                virtual_keys: vec![KEY_B],
            },
            "B".to_owned(),
        )]));

        assert_eq!(state.process(KEY_A, KeyTransition::Up, false), None);
        assert_eq!(
            state.process(KEY_B, KeyTransition::Down, false),
            Some("B".to_owned())
        );
    }

    #[test]
    fn matches_multi_key_chords_when_the_last_required_key_is_pressed() {
        let mut state = KeyboardState::new(
            BTreeMap::from([(
                NativeChord {
                    modifiers: 0,
                    virtual_keys: vec![KEY_A, KEY_B],
                },
                "A+B".to_owned(),
            )]),
            BTreeMap::new(),
        );

        assert_eq!(state.process(KEY_A, KeyTransition::Down, false), None);
        assert_eq!(
            state.process(KEY_B, KeyTransition::Down, false),
            Some("A+B".to_owned())
        );
        assert_eq!(state.process(KEY_B, KeyTransition::Up, false), None);
        assert_eq!(state.process(KEY_A, KeyTransition::Up, false), None);
        assert_eq!(state.process(KEY_B, KeyTransition::Down, false), None);
        assert_eq!(
            state.process(KEY_A, KeyTransition::Down, false),
            Some("A+B".to_owned())
        );
    }

    #[test]
    fn matches_modifier_only_chords() {
        let mut state = KeyboardState::new(
            BTreeMap::from([(
                NativeChord {
                    modifiers: 1,
                    virtual_keys: Vec::new(),
                },
                "Ctrl".to_owned(),
            )]),
            BTreeMap::from([(CTRL_LEFT, 1), (CTRL_RIGHT, 1)]),
        );

        assert_eq!(
            state.process(CTRL_LEFT, KeyTransition::Down, false),
            Some("Ctrl".to_owned())
        );
        assert_eq!(state.process(CTRL_RIGHT, KeyTransition::Down, false), None);
    }
}
