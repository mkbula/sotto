use crate::audio::AudioRecorder;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use signal_hook::consts::SIGUSR1;
#[cfg(unix)]
use signal_hook::iterator::Signals;

pub enum SignalCommand {
	Toggle,
}

#[cfg(unix)]
pub fn start_signal_listener() -> mpsc::Receiver<SignalCommand> {
	let (tx, rx) = mpsc::channel();

	thread::spawn(move || {
		if let Ok(mut signals) = Signals::new([SIGUSR1]) {
			for _ in signals.forever() {
				let _ = tx.send(SignalCommand::Toggle);
			}
		}
	});

	rx
}

#[cfg(not(unix))]
pub fn start_signal_listener() -> mpsc::Receiver<SignalCommand> {
	let (_tx, rx) = mpsc::channel();
	rx
}

// Linux input event codes for Ctrl+V, as expected by `ydotool key`.
const PASTE_KEYS: [&str; 4] = ["29:1", "47:1", "47:0", "29:0"];

pub fn paste_text(text: &str) -> bool {
	if text.is_empty() {
		return true;
	}

	if type_with_wtype(text) {
		return true;
	}

	// GNOME/Mutter lacks the virtual-keyboard protocol wtype needs, so fall back
	// to clipboard + uinput paste, which preserves non-ASCII characters.
	if paste_with_clipboard(text) {
		return true;
	}

	eprintln!(
		"Text insertion failed: wtype and clipboard paste (wl-copy + ydotool) both failed. \
		 Is ydotoold running? Try: systemctl --user enable --now ydotool"
	);
	false
}

fn type_with_wtype(text: &str) -> bool {
	Command::new("wtype")
		.arg("--")
		.arg(text)
		.stderr(Stdio::null())
		.status()
		.map(|s| s.success())
		.unwrap_or(false)
}

fn paste_with_clipboard(text: &str) -> bool {
	if let Err(e) = copy_to_clipboard(text) {
		eprintln!("Failed to copy transcription to clipboard: {}", e);
		return false;
	}

	// Give the compositor a moment to register the new clipboard owner.
	thread::sleep(Duration::from_millis(50));

	match Command::new("ydotool").arg("key").args(PASTE_KEYS).status() {
		Ok(s) if s.success() => {
			eprintln!(
				"Pasted transcription via clipboard ({} chars)",
				text.chars().count()
			);
			true
		}
		Ok(s) => {
			eprintln!("ydotool paste failed with {}; text left in clipboard", s);
			false
		}
		Err(e) => {
			eprintln!("Failed to run ydotool: {}; text left in clipboard", e);
			false
		}
	}
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
	let mut child = Command::new("wl-copy")
		.stdin(Stdio::piped())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.spawn()
		.map_err(|e| e.to_string())?;

	if let Some(mut stdin) = child.stdin.take() {
		stdin
			.write_all(text.as_bytes())
			.map_err(|e| e.to_string())?;
	}

	let status = child.wait().map_err(|e| e.to_string())?;
	if status.success() {
		Ok(())
	} else {
		Err(format!("wl-copy exited with {}", status))
	}
}

pub struct RecordingSession {
	audio: AudioRecorder,
}

impl RecordingSession {
	pub fn start(device_name: Option<String>) -> Result<Self, String> {
		let audio = AudioRecorder::start(device_name.as_deref())?;
		Ok(Self { audio })
	}

	pub fn stop(self) -> Vec<f32> {
		self.audio.stop()
	}
}
