// SPDX-License-Identifier: GPL-3.0-only

//! EIS (Emulated Input Server) receiver for remote desktop input injection.
//!
//! Accepts input events from EIS clients (e.g., xdg-desktop-portal-cosmic
//! RemoteDesktop sessions) and injects them into the compositor's input stack
//! using the same Smithay APIs as the virtual keyboard handler.

use calloop::channel;
use reis::{
    eis,
    event::DeviceCapability,
    handshake::{self, EisHandshaker},
    request::{EisRequest, EisRequestConverter},
    PendingRequestResult,
};
use smithay::{
    backend::input::KeyState,
    input::keyboard::{FilterResult, Keycode},
    utils::SERIAL_COUNTER,
};
use std::os::unix::net::UnixStream;
use tracing::{debug, error, info, warn};

use crate::state::State;

/// Events sent from the EIS processing thread to the compositor's calloop.
#[derive(Debug)]
pub enum EisInputEvent {
    /// Keyboard key press/release (evdev keycode)
    KeyboardKey { keycode: u32, pressed: bool },
    /// Relative pointer motion
    PointerMotion { dx: f64, dy: f64 },
    /// Absolute pointer motion (for absolute positioning devices)
    PointerMotionAbsolute { x: f64, y: f64 },
    /// Mouse button press/release (linux button code, e.g., BTN_LEFT=0x110)
    Button { button: u32, pressed: bool },
    /// Scroll delta (smooth scrolling)
    Scroll { dx: f64, dy: f64 },
    /// Client disconnected
    Disconnected,
}

/// Manages EIS connections and routes input events to the compositor.
pub struct EisState {
    /// Channel sender to inject events into calloop
    pub tx: channel::Sender<EisInputEvent>,
}

impl EisState {
    /// Create a new EIS state and register the event channel with calloop.
    ///
    /// Returns the `EisState` which can accept new socket connections.
    pub fn new(evlh: &calloop::LoopHandle<'static, State>) -> anyhow::Result<Self> {
        let (tx, rx) = channel::channel::<EisInputEvent>();

        evlh.insert_source(rx, |event, _, state| {
            if let channel::Event::Msg(eis_event) = event {
                process_eis_event(state, eis_event);
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert EIS channel source: {}", e.error))?;

        info!("EIS input receiver initialized");
        Ok(Self { tx })
    }

    /// Accept a new EIS client connection from a UNIX socket fd.
    ///
    /// Spawns a background thread that reads EIS protocol events from the
    /// socket and forwards them as `EisInputEvent` through the calloop channel.
    pub fn add_connection(&self, socket: UnixStream) {
        let tx = self.tx.clone();
        info!("Accepting new EIS client connection");

        std::thread::spawn(move || {
            if let Err(err) = run_eis_server(socket, tx) {
                error!("EIS server error: {}", err);
            }
        });
    }
}

/// Perform the EIS server-side handshake (blocking).
fn eis_handshake(
    context: &eis::Context,
) -> Result<handshake::EisHandshakeResp, reis::Error> {
    let mut handshaker = EisHandshaker::new(context, 0);
    // Flush the initial handshake version message
    context.flush().map_err(|e| reis::Error::Io(e.into()))?;

    loop {
        // Block until data is available
        rustix::event::poll(
            &mut [rustix::event::PollFd::new(context, rustix::event::PollFlags::IN)],
            -1,
        )
        .map_err(|e| reis::Error::Io(e.into()))?;

        context.read().map_err(reis::Error::Io)?;

        while let Some(result) = context.pending_request() {
            let request = match result {
                PendingRequestResult::Request(r) => r,
                PendingRequestResult::ParseError(e) => return Err(reis::Error::Parse(e)),
                PendingRequestResult::InvalidObject(id) => {
                    return Err(handshake::HandshakeError::InvalidObject(id).into());
                }
            };

            if let Some(resp) = handshaker
                .handle_request(request)
                .map_err(reis::Error::Handshake)?
            {
                return Ok(resp);
            }
        }
    }
}

/// Run the EIS server protocol on a socket, forwarding events to the compositor.
fn run_eis_server(
    socket: UnixStream,
    tx: channel::Sender<EisInputEvent>,
) -> anyhow::Result<()> {
    let context = eis::Context::new(socket)?;

    // Perform server-side handshake
    let handshake_resp = eis_handshake(&context)?;
    info!(
        "EIS handshake complete, client={:?}, context_type={:?}",
        handshake_resp.name, handshake_resp.context_type
    );

    // Create the request converter which manages seats, devices, and events
    let mut converter = EisRequestConverter::new(&context, handshake_resp, 0);

    // Add a seat with keyboard + pointer capabilities.
    // The seat is stored in the converter's connection state; _seat keeps it alive.
    let _seat = converter.handle().add_seat(
        Some("seat0"),
        &[
            DeviceCapability::Keyboard,
            DeviceCapability::Pointer,
            DeviceCapability::PointerAbsolute,
            DeviceCapability::Button,
            DeviceCapability::Scroll,
        ],
    );
    converter.handle().flush()?;

    // Main event loop
    loop {
        // Block until data is available
        rustix::event::poll(
            &mut [rustix::event::PollFd::new(&context, rustix::event::PollFlags::IN)],
            -1,
        )
        .map_err(|e| std::io::Error::from(e))?;

        let bytes_read = context.read()?;
        if bytes_read == 0 {
            info!("EIS connection closed (EOF)");
            let _ = tx.send(EisInputEvent::Disconnected);
            break;
        }

        // Process raw protocol requests through the converter
        while let Some(result) = context.pending_request() {
            let raw_request = match result {
                PendingRequestResult::Request(r) => r,
                PendingRequestResult::ParseError(e) => {
                    warn!("EIS parse error: {}", e);
                    continue;
                }
                PendingRequestResult::InvalidObject(id) => {
                    debug!("EIS invalid object: {}", id);
                    continue;
                }
            };

            if let Err(err) = converter.handle_request(raw_request) {
                warn!("EIS request handling error: {}", err);
                continue;
            }
        }

        // Drain high-level events from the converter
        while let Some(eis_request) = converter.next_request() {
            match eis_request {
                EisRequest::KeyboardKey(key_evt) => {
                    let pressed = key_evt.state == eis::keyboard::KeyState::Press;
                    if tx
                        .send(EisInputEvent::KeyboardKey {
                            keycode: key_evt.key,
                            pressed,
                        })
                        .is_err()
                    {
                        debug!("Compositor channel closed");
                        return Ok(());
                    }
                }
                EisRequest::PointerMotion(motion) => {
                    if tx
                        .send(EisInputEvent::PointerMotion {
                            dx: f64::from(motion.dx),
                            dy: f64::from(motion.dy),
                        })
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                EisRequest::PointerMotionAbsolute(motion) => {
                    if tx
                        .send(EisInputEvent::PointerMotionAbsolute {
                            x: f64::from(motion.dx_absolute),
                            y: f64::from(motion.dy_absolute),
                        })
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                EisRequest::Button(btn) => {
                    let pressed = btn.state == eis::button::ButtonState::Press;
                    if tx
                        .send(EisInputEvent::Button {
                            button: btn.button,
                            pressed,
                        })
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                EisRequest::ScrollDelta(scroll) => {
                    if tx
                        .send(EisInputEvent::Scroll {
                            dx: f64::from(scroll.dx),
                            dy: f64::from(scroll.dy),
                        })
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                EisRequest::Disconnect => {
                    info!("EIS client disconnected");
                    let _ = tx.send(EisInputEvent::Disconnected);
                    return Ok(());
                }
                EisRequest::Bind(bind) => {
                    // Client bound to seat capabilities - add device and resume
                    let capabilities =
                        capabilities_from_mask(bind.capabilities);
                    debug!("EIS client bound with capabilities: {:?}", capabilities);
                    let device = bind.seat.add_device(
                        Some("remote-input"),
                        eis::device::DeviceType::Virtual,
                        &capabilities,
                        |_| {},
                    );
                    device.resumed();
                    converter.handle().flush().ok();
                }
                EisRequest::DeviceStartEmulating(_) | EisRequest::DeviceStopEmulating(_) => {
                    // Acknowledged implicitly
                }
                EisRequest::Frame(_) => {
                    // Frame boundaries - we process events individually
                }
                _ => {
                    debug!("Unhandled EIS request: {:?}", eis_request);
                }
            }
        }
    }

    Ok(())
}

/// Convert a capability bitmask to a list of DeviceCapability values.
fn capabilities_from_mask(mask: u64) -> Vec<DeviceCapability> {
    let mut caps = Vec::new();
    for cap in [
        DeviceCapability::Pointer,
        DeviceCapability::PointerAbsolute,
        DeviceCapability::Keyboard,
        DeviceCapability::Touch,
        DeviceCapability::Scroll,
        DeviceCapability::Button,
    ] {
        if mask & (2 << cap as u64) != 0 {
            caps.push(cap);
        }
    }
    caps
}

/// Process a single EIS input event by injecting it into the compositor's
/// Smithay input stack.
fn process_eis_event(state: &mut State, event: EisInputEvent) {
    let time = state.common.clock.now().as_millis() as u32;

    match event {
        EisInputEvent::KeyboardKey { keycode, pressed } => {
            let seat = state.common.shell.read().seats.last_active().clone();
            if let Some(keyboard) = seat.get_keyboard() {
                let serial = SERIAL_COUNTER.next_serial();
                let key_state = if pressed {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };
                keyboard.input(
                    state,
                    Keycode::new(keycode),
                    key_state,
                    serial,
                    time,
                    |_, _, _| FilterResult::Forward::<bool>,
                );
            }
        }
        EisInputEvent::PointerMotion { dx, dy } => {
            let seat = state.common.shell.read().seats.last_active().clone();
            if let Some(pointer) = seat.get_pointer() {
                let current = pointer.current_location();
                let new_location = (current.x + dx, current.y + dy).into();
                let serial = SERIAL_COUNTER.next_serial();
                pointer.motion(
                    state,
                    None,
                    &smithay::input::pointer::MotionEvent {
                        location: new_location,
                        serial,
                        time,
                    },
                );
                pointer.frame(state);
            }
        }
        EisInputEvent::PointerMotionAbsolute { x, y } => {
            let seat = state.common.shell.read().seats.last_active().clone();
            if let Some(pointer) = seat.get_pointer() {
                let serial = SERIAL_COUNTER.next_serial();
                pointer.motion(
                    state,
                    None,
                    &smithay::input::pointer::MotionEvent {
                        location: (x, y).into(),
                        serial,
                        time,
                    },
                );
                pointer.frame(state);
            }
        }
        EisInputEvent::Button { button, pressed } => {
            let seat = state.common.shell.read().seats.last_active().clone();
            if let Some(pointer) = seat.get_pointer() {
                let serial = SERIAL_COUNTER.next_serial();
                let state_val = if pressed {
                    smithay::backend::input::ButtonState::Pressed
                } else {
                    smithay::backend::input::ButtonState::Released
                };
                pointer.button(
                    state,
                    &smithay::input::pointer::ButtonEvent {
                        button,
                        state: state_val,
                        serial,
                        time,
                    },
                );
                pointer.frame(state);
            }
        }
        EisInputEvent::Scroll { dx, dy } => {
            let seat = state.common.shell.read().seats.last_active().clone();
            if let Some(pointer) = seat.get_pointer() {
                use smithay::backend::input::Axis;
                let mut frame = smithay::input::pointer::AxisFrame::new(time);
                if dy.abs() > 0.0 {
                    frame = frame.value(Axis::Vertical, dy);
                }
                if dx.abs() > 0.0 {
                    frame = frame.value(Axis::Horizontal, dx);
                }
                pointer.axis(state, frame);
                pointer.frame(state);
            }
        }
        EisInputEvent::Disconnected => {
            info!("EIS client disconnected, cleaning up");
        }
    }
}
