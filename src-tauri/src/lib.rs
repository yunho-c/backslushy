use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct MacosLauncherSurface {
    panel: usize,
    view: usize,
}

#[cfg(target_os = "macos")]
static MACOS_LAUNCHER_SURFACE: std::sync::Mutex<Option<MacosLauncherSurface>> =
    std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
fn macos_launcher_surface(
) -> Option<(&'static objc2_app_kit::NSPanel, &'static objc2_app_kit::NSView)> {
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
            NSBackingStoreType, NSColor, NSFloatingWindowLevel, NSPanel, NSResponder,
            NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
        };

        let Some(_mtm) = MainThreadMarker::new() else {
            return;
        };
        let ns_window = unsafe { &*(ns_window.cast::<NSWindow>()) };
        let ns_view = unsafe { &*(ns_view.cast::<NSView>()) };
        let frame = ns_window.frame();

        let panel = unsafe {
            let allocated: objc2::rc::Allocated<NSPanel> =
                msg_send![launcher_panel_class(), alloc];
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
    if let Some((panel, _)) = macos_launcher_surface() {
        panel.center();
    }

    #[cfg(not(target_os = "macos"))]
    let _ = window.center();

    #[cfg(not(target_os = "macos"))]
    let _ = window.show();

    #[cfg(not(target_os = "macos"))]
    let _ = window.unminimize();

    focus_launcher_window(window);
    let _ = window.emit("launcher-shown", ());
}

#[cfg(target_os = "macos")]
fn hide_launcher(_window: &WebviewWindow) {
    log_focus_snapshot("before-hide");
    if let Some((panel, _)) = macos_launcher_surface() {
        panel.orderOut(None);
    }
    log_focus_snapshot("after-hide");
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
            app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    let _ = toggle_launcher(app);
                }
            })?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hide_launcher_command,
            focus_diagnostics_enabled,
            log_frontend_focus_diagnostic
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
