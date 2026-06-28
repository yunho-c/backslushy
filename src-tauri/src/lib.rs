use tauri::{AppHandle, Manager, WebviewWindow};
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

#[cfg(target_os = "macos")]
fn configure_macos_launcher_window(app: &tauri::App, window: &WebviewWindow) {
    let _ = app
        .handle()
        .set_activation_policy(tauri::ActivationPolicy::Accessory);
    let _ = app.handle().set_dock_visibility(false);

    if let (Ok(ns_window), Ok(ns_view)) = (window.ns_window(), window.ns_view()) {
        use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
        use objc2_app_kit::{
            NSBackingStoreType, NSColor, NSFloatingWindowLevel, NSPanel, NSResponder,
            NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
        };

        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let ns_window = unsafe { &*(ns_window.cast::<NSWindow>()) };
        let ns_view = unsafe { &*(ns_view.cast::<NSView>()) };
        let frame = ns_window.frame();

        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            frame,
            NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            false,
        );

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
    }
}

#[cfg(not(target_os = "macos"))]
fn focus_launcher_window(window: &WebviewWindow) {
    let _ = window.set_focus();
}

fn show_launcher(window: &WebviewWindow) {
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
}

#[cfg(target_os = "macos")]
fn hide_launcher(_window: &WebviewWindow) {
    if let Some((panel, _)) = macos_launcher_surface() {
        panel.orderOut(None);
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
    let window = main_window(&app)?;
    hide_launcher(&window);
    Ok(())
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
        .invoke_handler(tauri::generate_handler![hide_launcher_command])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
