use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest};
use enigo::{Button, Direction, Enigo, Key, Keyboard, Mouse};

use crate::desktop_actions::{config::failed_error, mouse::NormalizedMouseButton};

use super::{InputAction, native_input, native_input_raw};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum InputId {
    Key(String),
    Mouse(String),
}

#[derive(Debug, Clone, Copy)]
enum NativeInput {
    Key(Key),
    Mouse(Button),
}

impl NativeInput {
    fn change(self, enigo: &mut Enigo, direction: Direction) -> Result<(), String> {
        match self {
            Self::Key(key) => enigo.key(key, direction),
            Self::Mouse(button) => enigo.button(button, direction),
        }
        .map_err(|source| source.to_string())
    }
}

#[derive(Debug)]
struct HeldInput {
    native: NativeInput,
    owners: BTreeSet<String>,
}

#[derive(Debug, Default)]
struct HeldInputRegistry {
    inputs: BTreeMap<InputId, HeldInput>,
    runs: BTreeMap<String, Vec<InputId>>,
}

#[derive(Debug, Default)]
pub(crate) struct NativeInputState {
    registry: Mutex<HeldInputRegistry>,
}

impl NativeInputState {
    pub(crate) fn keyboard(
        &self,
        request: &RuntimeActionRequest,
        run_id: &str,
        canonical_keys: &[String],
        keys: &[Key],
        action: InputAction,
    ) -> Result<(), RuntimeActionError> {
        let inputs = canonical_keys
            .iter()
            .cloned()
            .zip(keys.iter().copied())
            .map(|(name, key)| (InputId::Key(name), NativeInput::Key(key)))
            .collect::<Vec<_>>();
        match action {
            InputAction::Press => self.press_key_combo(request, &inputs),
            InputAction::Down => self.hold_inputs(request, run_id, &inputs),
            InputAction::Up => self.release_inputs(request, run_id, &inputs),
        }
    }

    pub(crate) fn mouse(
        &self,
        request: &RuntimeActionRequest,
        run_id: &str,
        button: NormalizedMouseButton,
        action: InputAction,
        click_count: usize,
    ) -> Result<(), RuntimeActionError> {
        let input = (
            InputId::Mouse(button.name.to_owned()),
            NativeInput::Mouse(button.token),
        );
        match action {
            InputAction::Press => self.click_mouse(request, &input, click_count),
            InputAction::Down => self.hold_inputs(request, run_id, &[input]),
            InputAction::Up => self.release_inputs(request, run_id, &[input]),
        }
    }

    pub(crate) fn release_run(&self, run_id: &str) -> Result<(), String> {
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| "native input state lock is poisoned".to_owned())?;
        let Some(inputs) = registry.runs.get(run_id).cloned() else {
            return Ok(());
        };
        let mut enigo = native_input_raw()
            .map_err(|source| format!("native input init failed during run cleanup: {source}"))?;
        let mut first_error = None;
        for input in inputs.iter().rev() {
            if let Err(error) = release_owned_input(&mut registry, &mut enigo, run_id, input)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        if registry
            .runs
            .get(run_id)
            .is_some_and(std::vec::Vec::is_empty)
        {
            registry.runs.remove(run_id);
        }
        first_error.map_or(Ok(()), Err)
    }

    fn press_key_combo(
        &self,
        request: &RuntimeActionRequest,
        inputs: &[(InputId, NativeInput)],
    ) -> Result<(), RuntimeActionError> {
        let Some((last, held)) = inputs.split_last() else {
            return Err(failed_error(request, "key chord is empty"));
        };
        let registry = self.lock_registry(request)?;
        if registry.inputs.contains_key(&last.0) {
            return Err(failed_error(
                request,
                "cannot press and release a key that is currently held; release it first",
            ));
        }
        let mut enigo = native_input(request)?;
        let mut temporarily_pressed = Vec::new();
        for (id, native) in held {
            if registry.inputs.contains_key(id) {
                continue;
            }
            if let Err(source) = native.change(&mut enigo, Direction::Press) {
                let cleanup = release_temporary(&mut enigo, &temporarily_pressed);
                return Err(input_failure(request, "key press", source, cleanup));
            }
            temporarily_pressed.push(*native);
        }
        let click_error = last.1.change(&mut enigo, Direction::Click).err();
        let cleanup = release_temporary(&mut enigo, &temporarily_pressed);
        drop(registry);
        if let Some(source) = click_error {
            return Err(input_failure(request, "key press", source, cleanup));
        }
        if let Some(source) = cleanup {
            return Err(failed_error(
                request,
                format!("key release failed: {source}"),
            ));
        }
        Ok(())
    }

    fn click_mouse(
        &self,
        request: &RuntimeActionRequest,
        input: &(InputId, NativeInput),
        click_count: usize,
    ) -> Result<(), RuntimeActionError> {
        let registry = self.lock_registry(request)?;
        if registry.inputs.contains_key(&input.0) {
            return Err(failed_error(
                request,
                "cannot click a mouse button that is currently held; release it first",
            ));
        }
        let mut enigo = native_input(request)?;
        for _ in 0..click_count {
            input
                .1
                .change(&mut enigo, Direction::Click)
                .map_err(|source| failed_error(request, format!("mouse click failed: {source}")))?;
        }
        Ok(())
    }

    fn hold_inputs(
        &self,
        request: &RuntimeActionRequest,
        run_id: &str,
        inputs: &[(InputId, NativeInput)],
    ) -> Result<(), RuntimeActionError> {
        let mut registry = self.lock_registry(request)?;
        let mut enigo = native_input(request)?;
        let mut acquired = Vec::new();
        for (id, native) in inputs {
            match acquire_input(&mut registry, &mut enigo, run_id, id.clone(), *native) {
                Ok(true) => acquired.push(id.clone()),
                Ok(false) => {}
                Err(source) => {
                    for acquired_id in acquired.iter().rev() {
                        let _ = release_owned_input(&mut registry, &mut enigo, run_id, acquired_id);
                    }
                    return Err(failed_error(
                        request,
                        format!("input press failed: {source}"),
                    ));
                }
            }
        }
        Ok(())
    }

    fn release_inputs(
        &self,
        request: &RuntimeActionRequest,
        run_id: &str,
        inputs: &[(InputId, NativeInput)],
    ) -> Result<(), RuntimeActionError> {
        let mut registry = self.lock_registry(request)?;
        let mut enigo = native_input(request)?;
        for (id, _) in inputs.iter().rev() {
            release_owned_input(&mut registry, &mut enigo, run_id, id).map_err(|source| {
                failed_error(request, format!("input release failed: {source}"))
            })?;
        }
        Ok(())
    }

    fn lock_registry(
        &self,
        request: &RuntimeActionRequest,
    ) -> Result<std::sync::MutexGuard<'_, HeldInputRegistry>, RuntimeActionError> {
        self.registry
            .lock()
            .map_err(|_| failed_error(request, "native input state lock is poisoned"))
    }
}

fn acquire_input(
    registry: &mut HeldInputRegistry,
    enigo: &mut Enigo,
    run_id: &str,
    id: InputId,
    native: NativeInput,
) -> Result<bool, String> {
    if let Some(held) = registry.inputs.get_mut(&id) {
        if !held.owners.insert(run_id.to_owned()) {
            return Ok(false);
        }
    } else {
        native.change(enigo, Direction::Press)?;
        registry.inputs.insert(
            id.clone(),
            HeldInput {
                native,
                owners: BTreeSet::from([run_id.to_owned()]),
            },
        );
    }
    registry.runs.entry(run_id.to_owned()).or_default().push(id);
    Ok(true)
}

fn release_owned_input(
    registry: &mut HeldInputRegistry,
    enigo: &mut Enigo,
    run_id: &str,
    id: &InputId,
) -> Result<(), String> {
    let Some(held) = registry.inputs.get(id) else {
        remove_run_input(registry, run_id, id);
        return Ok(());
    };
    if !held.owners.contains(run_id) {
        return Ok(());
    }
    let is_last_owner = held.owners.len() == 1;
    let native = held.native;
    if is_last_owner {
        native.change(enigo, Direction::Release)?;
        registry.inputs.remove(id);
    } else if let Some(held) = registry.inputs.get_mut(id) {
        held.owners.remove(run_id);
    }
    remove_run_input(registry, run_id, id);
    Ok(())
}

fn remove_run_input(registry: &mut HeldInputRegistry, run_id: &str, id: &InputId) {
    if let Some(inputs) = registry.runs.get_mut(run_id) {
        inputs.retain(|candidate| candidate != id);
        if inputs.is_empty() {
            registry.runs.remove(run_id);
        }
    }
}

fn release_temporary(enigo: &mut Enigo, inputs: &[NativeInput]) -> Option<String> {
    let mut first_error = None;
    for input in inputs.iter().rev() {
        if let Err(source) = input.change(enigo, Direction::Release)
            && first_error.is_none()
        {
            first_error = Some(source);
        }
    }
    first_error
}

fn input_failure(
    request: &RuntimeActionRequest,
    operation: &str,
    source: String,
    cleanup: Option<String>,
) -> RuntimeActionError {
    let detail = cleanup.map_or(source.clone(), |cleanup_error| {
        format!("{source}; input cleanup also failed: {cleanup_error}")
    });
    failed_error(request, format!("{operation} failed: {detail}"))
}
