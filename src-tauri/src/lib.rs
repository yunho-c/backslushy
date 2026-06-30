use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
const LAUNCHER_OPEN_ANIMATION_MS: u64 = 60;
#[cfg(target_os = "macos")]
const LAUNCHER_CLOSE_ANIMATION_MS: u64 = 110;
#[cfg(target_os = "macos")]
const LAUNCHER_ANIMATION_FRAME_MS: u64 = 16;
#[cfg(target_os = "macos")]
const PASTE_DISPATCH_DELAY_MS: u64 = LAUNCHER_CLOSE_ANIMATION_MS + 55;
#[cfg(target_os = "macos")]
const LAUNCHER_MIN_HEIGHT: f64 = 220.0;
#[cfg(target_os = "macos")]
const LAUNCHER_MAX_HEIGHT: f64 = 560.0;
#[cfg(target_os = "macos")]
const LAUNCHER_SCREEN_MARGIN: f64 = 80.0;

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct MacosLauncherSurface {
    panel: usize,
    view: usize,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct LauncherAnchor {
    x: f64,
    top_y: f64,
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct PasteboardSnapshot {
    items: Vec<PasteboardSnapshotItem>,
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct PasteboardSnapshotItem {
    entries: Vec<PasteboardSnapshotEntry>,
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct PasteboardSnapshotEntry {
    type_name: String,
    data: Vec<u8>,
}

#[cfg(target_os = "macos")]
static MACOS_LAUNCHER_SURFACE: std::sync::Mutex<Option<MacosLauncherSurface>> =
    std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
static SAVED_LAUNCHER_ANCHOR: std::sync::Mutex<Option<LauncherAnchor>> =
    std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
static LAUNCHER_ANIMATION_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(target_os = "macos")]
fn macos_launcher_surface() -> Option<(
    &'static objc2_app_kit::NSPanel,
    &'static objc2_app_kit::NSView,
)> {
    let surface = *MACOS_LAUNCHER_SURFACE.lock().ok()?;
    surface.map(|surface| unsafe {
        (
            &*(surface.panel as *mut objc2_app_kit::NSPanel),
            &*(surface.view as *mut objc2_app_kit::NSView),
        )
    })
}

fn focus_diagnostics_enabled_flag() -> bool {
    std::env::var("BKSLASH_FOCUS_DIAGNOSTICS")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> std::ffi::c_uchar;
}

#[cfg(target_os = "macos")]
fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

#[cfg(target_os = "macos")]
fn next_launcher_animation_generation() -> u64 {
    LAUNCHER_ANIMATION_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

#[cfg(target_os = "macos")]
fn launcher_animation_generation() -> u64 {
    LAUNCHER_ANIMATION_GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(target_os = "macos")]
fn ease_out_cubic(progress: f64) -> f64 {
    1.0 - (1.0 - progress).powi(3)
}

#[cfg(target_os = "macos")]
fn animate_launcher_alpha(
    window: &WebviewWindow,
    generation: u64,
    from_alpha: f64,
    to_alpha: f64,
    duration_ms: u64,
    order_out_on_complete: bool,
) {
    let window = window.clone();

    std::thread::spawn(move || {
        let frame_count = (duration_ms / LAUNCHER_ANIMATION_FRAME_MS).max(1);

        for frame in 1..=frame_count {
            std::thread::sleep(std::time::Duration::from_millis(duration_ms / frame_count));

            let progress = frame as f64 / frame_count as f64;
            let eased = ease_out_cubic(progress);
            let alpha = from_alpha + (to_alpha - from_alpha) * eased;
            let window = window.clone();

            let _ = window.run_on_main_thread(move || {
                if launcher_animation_generation() != generation {
                    return;
                }

                if let Some((panel, _)) = macos_launcher_surface() {
                    panel.setAlphaValue(alpha);
                }
            });
        }

        if order_out_on_complete {
            let completion_window = window.clone();
            let _ = window.run_on_main_thread(move || {
                if launcher_animation_generation() != generation {
                    return;
                }

                log_focus_snapshot("before-order-out");
                if let Some((panel, _)) = macos_launcher_surface() {
                    panel.orderOut(None);
                    panel.setAlphaValue(0.0);
                }
                log_focus_snapshot("after-hide");
                let _ = completion_window.emit("launcher-hidden", ());
            });
        }
    });
}

#[cfg(target_os = "macos")]
fn clamp_axis(value: f64, min: f64, max: f64) -> f64 {
    if max < min {
        min
    } else {
        value.clamp(min, max)
    }
}

#[cfg(target_os = "macos")]
fn point_inside_rect(x: f64, y: f64, rect: &objc2_foundation::NSRect) -> bool {
    x >= rect.origin.x
        && x <= rect.origin.x + rect.size.width
        && y >= rect.origin.y
        && y <= rect.origin.y + rect.size.height
}

#[cfg(target_os = "macos")]
fn visible_frame_for_launcher_frame(
    frame: objc2_foundation::NSRect,
) -> Option<objc2_foundation::NSRect> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;

    let Some(mtm) = MainThreadMarker::new() else {
        return None;
    };

    let center_x = frame.origin.x + frame.size.width / 2.0;
    let center_y = frame.origin.y + frame.size.height / 2.0;
    let screens = NSScreen::screens(mtm);

    (0..screens.count())
        .map(|index| screens.objectAtIndex(index).visibleFrame())
        .find(|visible_frame| point_inside_rect(center_x, center_y, visible_frame))
        .or_else(|| NSScreen::mainScreen(mtm).map(|screen| screen.visibleFrame()))
}

#[cfg(target_os = "macos")]
fn clamp_launcher_frame(frame: objc2_foundation::NSRect) -> objc2_foundation::NSRect {
    let Some(visible_frame) = visible_frame_for_launcher_frame(frame) else {
        return frame;
    };

    let mut clamped_frame = frame;
    clamped_frame.origin.x = clamp_axis(
        frame.origin.x,
        visible_frame.origin.x,
        visible_frame.origin.x + visible_frame.size.width - frame.size.width,
    );
    clamped_frame.origin.y = clamp_axis(
        frame.origin.y,
        visible_frame.origin.y,
        visible_frame.origin.y + visible_frame.size.height - frame.size.height,
    );
    clamped_frame
}

#[cfg(target_os = "macos")]
fn clamp_launcher_height(height: f64, frame: objc2_foundation::NSRect) -> f64 {
    let screen_max_height = visible_frame_for_launcher_frame(frame)
        .map(|visible_frame| visible_frame.size.height - LAUNCHER_SCREEN_MARGIN)
        .unwrap_or(LAUNCHER_MAX_HEIGHT);
    height.clamp(
        LAUNCHER_MIN_HEIGHT,
        LAUNCHER_MAX_HEIGHT.min(screen_max_height).max(LAUNCHER_MIN_HEIGHT),
    )
}

#[cfg(target_os = "macos")]
fn launcher_frame_for_height(
    panel: &objc2_app_kit::NSPanel,
    height: f64,
) -> objc2_foundation::NSRect {
    let mut frame = panel.frame();
    let top_y = frame.origin.y + frame.size.height;
    let height = clamp_launcher_height(height, frame);
    frame.origin.y = top_y - height;
    frame.size.height = height;
    clamp_launcher_frame(frame)
}

#[cfg(target_os = "macos")]
fn apply_launcher_frame(
    panel: &objc2_app_kit::NSPanel,
    ns_view: &objc2_app_kit::NSView,
    frame: objc2_foundation::NSRect,
    animated: bool,
) {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let view_frame = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(frame.size.width, frame.size.height),
    );
    ns_view.setFrame(view_frame);
    panel.setFrame_display_animate(frame, true, animated);
}

#[cfg(target_os = "macos")]
fn resize_launcher_panel(height: f64, animated: bool) -> Result<(), String> {
    use objc2::MainThreadMarker;

    if MainThreadMarker::new().is_none() {
        return Err("launcher resize must run on the main thread".to_string());
    }

    let Some((panel, ns_view)) = macos_launcher_surface() else {
        return Err("launcher panel was not initialized".to_string());
    };

    let frame = launcher_frame_for_height(panel, height);
    apply_launcher_frame(panel, ns_view, frame, animated && panel.isVisible());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn resize_launcher_window(window: &WebviewWindow, height: f64) -> Result<(), String> {
    use tauri::{LogicalSize, Size};

    let current_size = window.inner_size().map_err(|error| error.to_string())?;
    let scale_factor = window.scale_factor().map_err(|error| error.to_string())?;
    let width = current_size.width as f64 / scale_factor;
    window
        .set_size(Size::Logical(LogicalSize::new(width, height.max(220.0))))
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn save_launcher_position(panel: &objc2_app_kit::NSPanel) {
    let frame = panel.frame();
    if let Ok(mut saved_anchor) = SAVED_LAUNCHER_ANCHOR.lock() {
        *saved_anchor = Some(LauncherAnchor {
            x: frame.origin.x,
            top_y: frame.origin.y + frame.size.height,
        });
    }
}

#[cfg(target_os = "macos")]
fn restore_launcher_position_or_center(panel: &objc2_app_kit::NSPanel) {
    let saved_anchor = SAVED_LAUNCHER_ANCHOR
        .lock()
        .ok()
        .and_then(|saved_anchor| *saved_anchor);

    if let Some(saved_anchor) = saved_anchor {
        let mut frame = panel.frame();
        frame.origin.x = saved_anchor.x;
        frame.origin.y = saved_anchor.top_y - frame.size.height;
        let frame = clamp_launcher_frame(frame);
        panel.setFrame_display(frame, true);
        return;
    }

    panel.center();
}

#[cfg(target_os = "macos")]
fn objc_class_name<T: ?Sized>(object: &T) -> String {
    let object = unsafe { &*(object as *const T as *const objc2::runtime::AnyObject) };
    object.class().name().to_string_lossy().into_owned()
}

#[cfg(target_os = "macos")]
fn object_addr<T: ?Sized>(object: &T) -> usize {
    object as *const T as *const () as usize
}

#[cfg(target_os = "macos")]
fn log_focus_snapshot(stage: &str) {
    if !focus_diagnostics_enabled_flag() {
        return;
    }

    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSRunningApplication, NSWorkspace};

    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[bkslash:focus] stage={stage} skipped=not-main-thread");
        return;
    };

    let app = NSApplication::sharedApplication(mtm);
    let key_window = app.keyWindow();
    let main_window = app.mainWindow();
    let surface = macos_launcher_surface();
    let panel_addr = surface
        .map(|(panel, _)| object_addr(panel))
        .unwrap_or_default();

    let key_window_addr = key_window.as_deref().map(object_addr).unwrap_or_default();
    let main_window_addr = main_window.as_deref().map(object_addr).unwrap_or_default();
    let key_window_class = key_window
        .as_deref()
        .map(objc_class_name)
        .unwrap_or_else(|| "none".to_string());
    let main_window_class = main_window
        .as_deref()
        .map(objc_class_name)
        .unwrap_or_else(|| "none".to_string());

    let (
        panel_visible,
        panel_key,
        panel_main,
        panel_can_key,
        panel_can_main,
        panel_first_responder_class,
    ) = surface
        .map(|(panel, _)| {
            (
                panel.isVisible(),
                panel.isKeyWindow(),
                panel.isMainWindow(),
                panel.canBecomeKeyWindow(),
                panel.canBecomeMainWindow(),
                panel
                    .firstResponder()
                    .as_deref()
                    .map(objc_class_name)
                    .unwrap_or_else(|| "none".to_string()),
            )
        })
        .unwrap_or((false, false, false, false, false, "none".to_string()));

    let frontmost = NSWorkspace::sharedWorkspace().frontmostApplication();
    let frontmost_name = frontmost
        .as_deref()
        .and_then(NSRunningApplication::localizedName)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string());
    let frontmost_bundle = frontmost
        .as_deref()
        .and_then(NSRunningApplication::bundleIdentifier)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string());

    eprintln!(
        "[bkslash:focus] stage={stage} app_active={} key_window_class={} key_window_is_panel={} key_window=0x{:x} main_window_class={} main_window_is_panel={} main_window=0x{:x} panel_visible={} panel_key={} panel_main={} panel_can_key={} panel_can_main={} panel_first_responder_class={} frontmost_name=\"{}\" frontmost_bundle=\"{}\"",
        app.isActive(),
        key_window_class,
        key_window_addr != 0 && key_window_addr == panel_addr,
        key_window_addr,
        main_window_class,
        main_window_addr != 0 && main_window_addr == panel_addr,
        main_window_addr,
        panel_visible,
        panel_key,
        panel_main,
        panel_can_key,
        panel_can_main,
        panel_first_responder_class,
        frontmost_name,
        frontmost_bundle,
    );
}

#[cfg(target_os = "macos")]
fn snapshot_general_pasteboard() -> PasteboardSnapshot {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSData;

    let pasteboard = NSPasteboard::generalPasteboard();
    let mut snapshot = PasteboardSnapshot { items: Vec::new() };

    let Some(items) = pasteboard.pasteboardItems() else {
        return snapshot;
    };

    for item_index in 0..items.count() {
        let item = items.objectAtIndex(item_index);
        let types = item.types();
        let mut snapshot_item = PasteboardSnapshotItem {
            entries: Vec::new(),
        };

        for type_index in 0..types.count() {
            let item_type = types.objectAtIndex(type_index);
            if let Some(data) = item.dataForType(&item_type) {
                snapshot_item.entries.push(PasteboardSnapshotEntry {
                    type_name: item_type.to_string(),
                    data: NSData::to_vec(&data),
                });
            }
        }

        if !snapshot_item.entries.is_empty() {
            snapshot.items.push(snapshot_item);
        }
    }

    snapshot
}

#[cfg(target_os = "macos")]
fn write_string_to_general_pasteboard(value: &str) -> Result<(), String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();

    let value = NSString::from_str(value);
    if pasteboard.setString_forType(&value, unsafe { NSPasteboardTypeString }) {
        Ok(())
    } else {
        Err("failed to write alias expansion to the system pasteboard".to_string())
    }
}

#[cfg(target_os = "macos")]
fn restore_general_pasteboard_if_unchanged(snapshot: PasteboardSnapshot, expected: &str) {
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{
        NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting,
    };
    use objc2_foundation::{NSArray, NSData, NSString};

    let pasteboard = NSPasteboard::generalPasteboard();
    let current_string = pasteboard
        .stringForType(unsafe { NSPasteboardTypeString })
        .map(|value| value.to_string());

    if current_string.as_deref() != Some(expected) {
        if focus_diagnostics_enabled_flag() {
            eprintln!(
                "[bkslash:focus] stage=clipboard-restore skipped=clipboard-changed current_string={:?}",
                current_string
            );
        }
        return;
    }

    if snapshot.items.is_empty() {
        pasteboard.clearContents();
        return;
    }

    let restored_items = snapshot
        .items
        .into_iter()
        .filter_map(|snapshot_item| {
            let pasteboard_item = NSPasteboardItem::new();
            let mut wrote_entry = false;

            for entry in snapshot_item.entries {
                let item_type = NSString::from_str(&entry.type_name);
                let data = NSData::with_bytes(&entry.data);
                wrote_entry |= pasteboard_item.setData_forType(&data, &item_type);
            }

            wrote_entry.then(|| ProtocolObject::from_retained(pasteboard_item))
        })
        .collect::<Vec<_>>();

    if restored_items.is_empty() {
        pasteboard.clearContents();
        return;
    }

    let restored_items =
        NSArray::<ProtocolObject<dyn NSPasteboardWriting>>::from_retained_slice(&restored_items);
    pasteboard.clearContents();
    pasteboard.writeObjects(&restored_items);
}

#[cfg(target_os = "macos")]
fn post_command_v() -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "failed to create a CoreGraphics event source".to_string())?;
    let command_flag =
        CGEventFlags::from_bits_retain(CGEventFlags::CGEventFlagCommand.bits() | 0x8);
    let key_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, true)
        .map_err(|_| "failed to create Cmd+V key-down event".to_string())?;
    let key_up = CGEvent::new_keyboard_event(source, KeyCode::ANSI_V, false)
        .map_err(|_| "failed to create Cmd+V key-up event".to_string())?;

    key_down.set_flags(command_flag);
    key_up.set_flags(command_flag);
    key_down.post(CGEventTapLocation::AnnotatedSession);
    key_up.post(CGEventTapLocation::AnnotatedSession);
    Ok(())
}

#[cfg(target_os = "macos")]
fn launcher_panel_class() -> &'static objc2::runtime::AnyClass {
    use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
    use objc2::{class, sel};
    use std::sync::OnceLock;

    extern "C" fn can_become_key_window(_this: &AnyObject, _sel: Sel) -> Bool {
        Bool::YES
    }

    extern "C" fn can_become_main_window(_this: &AnyObject, _sel: Sel) -> Bool {
        Bool::NO
    }

    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| unsafe {
        let superclass = class!(NSPanel);
        let mut builder = ClassBuilder::new(c"BkslashLauncherPanel", superclass)
            .expect("BkslashLauncherPanel class should register once");
        builder.add_method::<AnyObject, _>(
            sel!(canBecomeKeyWindow),
            can_become_key_window as extern "C" fn(_, _) -> _,
        );
        builder.add_method::<AnyObject, _>(
            sel!(canBecomeMainWindow),
            can_become_main_window as extern "C" fn(_, _) -> _,
        );
        builder.register()
    })
}

#[cfg(target_os = "macos")]
fn configure_macos_launcher_window(app: &tauri::App, window: &WebviewWindow) {
    let _ = app
        .handle()
        .set_activation_policy(tauri::ActivationPolicy::Accessory);
    let _ = app.handle().set_dock_visibility(false);

    if let (Ok(ns_window), Ok(ns_view)) = (window.ns_window(), window.ns_view()) {
        use objc2::{msg_send, rc::Retained, MainThreadMarker};
        use objc2_app_kit::{
            NSBackingStoreType, NSColor, NSFloatingWindowLevel, NSPanel, NSResponder, NSView,
            NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
        };

        let Some(_mtm) = MainThreadMarker::new() else {
            return;
        };
        let ns_window = unsafe { &*(ns_window.cast::<NSWindow>()) };
        let ns_view = unsafe { &*(ns_view.cast::<NSView>()) };
        let frame = ns_window.frame();

        let panel = unsafe {
            let allocated: objc2::rc::Allocated<NSPanel> = msg_send![launcher_panel_class(), alloc];
            NSPanel::initWithContentRect_styleMask_backing_defer(
                allocated,
                frame,
                NSWindowStyleMask::NonactivatingPanel,
                NSBackingStoreType::Buffered,
                false,
            )
        };

        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(false);
        panel.setWorksWhenModal(true);
        panel.setLevel(NSFloatingWindowLevel);
        panel.setHidesOnDeactivate(false);
        panel.setCanHide(false);
        panel.setHasShadow(true);
        panel.setOpaque(false);
        panel.setAlphaValue(0.0);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe {
            panel.setReleasedWhenClosed(false);
        }

        let mut view_frame = frame;
        view_frame.origin.x = 0.0;
        view_frame.origin.y = 0.0;
        ns_view.removeFromSuperview();
        ns_view.setFrame(view_frame);
        panel.setContentView(Some(ns_view));
        let _ = panel.makeFirstResponder(Some(AsRef::<NSResponder>::as_ref(ns_view)));
        ns_window.orderOut(None);

        if let Ok(mut surface) = MACOS_LAUNCHER_SURFACE.lock() {
            *surface = Some(MacosLauncherSurface {
                panel: Retained::into_raw(panel) as usize,
                view: ns_view as *const NSView as usize,
            });
        }

        log_focus_snapshot("after-panel-setup");
    }
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main window was not found".to_string())
}

#[cfg(target_os = "macos")]
fn focus_launcher_window(_window: &WebviewWindow) {
    if let Some((panel, ns_view)) = macos_launcher_surface() {
        use objc2_app_kit::NSResponder;

        let _ = panel.makeFirstResponder(Some(AsRef::<NSResponder>::as_ref(ns_view)));
        panel.makeKeyAndOrderFront(None);
        log_focus_snapshot("after-make-key");
    }
}

#[cfg(not(target_os = "macos"))]
fn focus_launcher_window(window: &WebviewWindow) {
    let _ = window.set_focus();
}

fn show_launcher(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    log_focus_snapshot("before-show");

    #[cfg(target_os = "macos")]
    let animation_generation = next_launcher_animation_generation();

    #[cfg(target_os = "macos")]
    let _ = window.emit("launcher-showing", ());

    #[cfg(target_os = "macos")]
    if let Some((panel, _)) = macos_launcher_surface() {
        restore_launcher_position_or_center(panel);
        panel.setAlphaValue(0.0);
    }

    #[cfg(not(target_os = "macos"))]
    let _ = window.center();

    #[cfg(not(target_os = "macos"))]
    let _ = window.show();

    #[cfg(not(target_os = "macos"))]
    let _ = window.unminimize();

    focus_launcher_window(window);
    let _ = window.emit("launcher-shown", ());

    #[cfg(target_os = "macos")]
    animate_launcher_alpha(
        window,
        animation_generation,
        0.0,
        1.0,
        LAUNCHER_OPEN_ANIMATION_MS,
        false,
    );
}

#[cfg(target_os = "macos")]
fn hide_launcher(window: &WebviewWindow) {
    let animation_generation = next_launcher_animation_generation();
    log_focus_snapshot("before-hide");
    let _ = window.emit("launcher-hiding", ());

    if let Some((panel, _)) = macos_launcher_surface() {
        save_launcher_position(panel);
        let from_alpha = panel.alphaValue();
        animate_launcher_alpha(
            window,
            animation_generation,
            from_alpha,
            0.0,
            LAUNCHER_CLOSE_ANIMATION_MS,
            true,
        );
    } else {
        log_focus_snapshot("after-hide");
        let _ = window.emit("launcher-hidden", ());
    }
}

#[cfg(not(target_os = "macos"))]
fn hide_launcher(window: &WebviewWindow) {
    let _ = window.hide();
}

#[cfg(target_os = "macos")]
fn launcher_is_visible(_window: &WebviewWindow) -> bool {
    macos_launcher_surface()
        .map(|(panel, _)| panel.isVisible())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn launcher_is_visible(window: &WebviewWindow) -> bool {
    window.is_visible().unwrap_or(false)
}

fn toggle_launcher(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;

    if launcher_is_visible(&window) {
        hide_launcher(&window);
    } else {
        show_launcher(&window);
    }

    Ok(())
}

#[tauri::command]
fn hide_launcher_command(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    log_focus_snapshot("hide-command");

    let window = main_window(&app)?;
    hide_launcher(&window);
    Ok(())
}

#[tauri::command]
fn set_launcher_height_command(
    app: AppHandle,
    height: f64,
    animated: bool,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        resize_launcher_panel(height, animated)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = animated;
        let window = main_window(&app)?;
        resize_launcher_window(&window, height)
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn start_launcher_drag_command() -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEvent, NSEventType};

    let Some(mtm) = MainThreadMarker::new() else {
        return Err("launcher drag must start on the main thread".to_string());
    };

    let Some((panel, _)) = macos_launcher_surface() else {
        return Err("launcher panel was not initialized".to_string());
    };

    let app = NSApplication::sharedApplication(mtm);
    let Some(event) = app.currentEvent() else {
        return Err("no current mouse event was available for launcher drag".to_string());
    };

    let drag_event = if event.r#type().0 == 0x15 {
        NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            NSEventType::LeftMouseDown,
            NSEvent::mouseLocation(),
            event.modifierFlags(),
            event.timestamp(),
            event.windowNumber(),
            None,
            event.eventNumber(),
            1,
            1.0,
        )
        .unwrap_or(event)
    } else {
        event
    };

    panel.performWindowDragWithEvent(&drag_event);
    save_launcher_position(panel);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn start_launcher_drag_command(app: AppHandle) -> Result<(), String> {
    let window = main_window(&app)?;
    window.start_dragging().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn paste_alias_command(
    app: AppHandle,
    expansion: String,
    restore_clipboard: bool,
) -> Result<(), String> {
    if !accessibility_trusted() {
        return Err(
            "bkslash needs macOS Accessibility permission before it can paste into other apps"
                .to_string(),
        );
    }

    let snapshot = restore_clipboard.then(snapshot_general_pasteboard);
    write_string_to_general_pasteboard(&expansion)?;

    let window = main_window(&app)?;
    hide_launcher(&window);

    if focus_diagnostics_enabled_flag() {
        eprintln!(
            "[bkslash:focus] stage=paste-dispatch scheduled=true restore_clipboard={restore_clipboard}"
        );
    }

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(PASTE_DISPATCH_DELAY_MS));

        if let Err(error) = post_command_v() {
            eprintln!("[bkslash:focus] stage=paste-dispatch error=\"{error}\"");
        } else if focus_diagnostics_enabled_flag() {
            eprintln!("[bkslash:focus] stage=paste-dispatch posted=true");
        }

        if let Some(snapshot) = snapshot {
            std::thread::sleep(std::time::Duration::from_millis(700));
            restore_general_pasteboard_if_unchanged(snapshot, &expansion);
            if focus_diagnostics_enabled_flag() {
                eprintln!("[bkslash:focus] stage=clipboard-restore completed=true");
            }
        }
    });

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn paste_alias_command(
    app: AppHandle,
    _expansion: String,
    _restore_clipboard: bool,
) -> Result<(), String> {
    let window = main_window(&app)?;
    hide_launcher(&window);
    Err("paste injection is currently implemented only on macOS".to_string())
}

#[tauri::command]
fn focus_diagnostics_enabled() -> bool {
    focus_diagnostics_enabled_flag()
}

#[tauri::command]
fn log_frontend_focus_diagnostic(stage: String, detail: serde_json::Value) {
    if focus_diagnostics_enabled_flag() {
        eprintln!("[bkslash:focus] stage={stage} frontend={detail}");
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                configure_macos_launcher_window(app, &window);
            }

            let shortcut = Shortcut::new(None, Code::Backslash);
            app.global_shortcut()
                .on_shortcut(shortcut, |app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = toggle_launcher(app);
                    }
                })?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hide_launcher_command,
            set_launcher_height_command,
            start_launcher_drag_command,
            paste_alias_command,
            focus_diagnostics_enabled,
            log_frontend_focus_diagnostic
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
