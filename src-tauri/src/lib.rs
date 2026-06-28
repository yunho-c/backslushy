use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main window was not found".to_string())
}

fn show_launcher(window: &WebviewWindow) {
    let _ = window.center();
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn hide_launcher(window: &WebviewWindow) {
    let _ = window.hide();
}

fn toggle_launcher(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;

    match window.is_visible() {
        Ok(true) => hide_launcher(&window),
        _ => show_launcher(&window),
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
