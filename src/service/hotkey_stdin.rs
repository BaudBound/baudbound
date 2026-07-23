use std::{
    io::{self, BufRead},
    sync::mpsc::Sender,
    thread,
};

pub(super) fn spawn_hotkey_stdin_reader(sender: Sender<String>) {
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(key) = line else {
                break;
            };
            let key = key.trim().to_owned();
            if key.is_empty() {
                continue;
            }
            if sender.send(key).is_err() {
                break;
            }
        }
    });
}
